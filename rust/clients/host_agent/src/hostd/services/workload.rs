/*
  This client is associated with the:
    - HPOS account
    - host user

  This client does not publish to any workload subjects.

  This client is responsible for subscribing to workload streams that handle:
    - installing new workloads onto the hosting device
    - removing workloads from the hosting device
    - sending workload status upon request
    - sending out active periodic workload reports
*/
use async_nats::jetstream::kv::Store;
use db_utils::schemas::workload::WorkloadStateDiscriminants;
use futures::{StreamExt, TryStreamExt};
use hpos_hal::inventory::MACHINE_ID_PATH;
use nats_utils::macros::ApiOptions;
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    jetstream_service::JsStreamService,
    types::{
        sanitization::sanitize_nats_name, HcHttpGwRequest, HcHttpGwResponse, HcHttpGwResponseMsg,
        JsServiceBuilder, ServiceConsumerBuilder, ServiceError,
    },
};
use reqwest::Method;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use url::Url;
use workload::{
    host_api::{HcHttpGwWorkerKvBucketValue, HostJobApi},
    types::{WorkloadError, WorkloadOpResult, WorkloadServiceSubjects},
    WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

use crate::hostd::services::utils::create_callback_subject;
use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

pub async fn run(
    host_client: Arc<RwLock<JsClient>>,
    host_id: &str,
    jetstream_domain: &str,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> HostAgentResult<()> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_id : {}", host_id);

    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
        maybe_source_js_domain: Some(jetstream_domain.to_string()),
    };

    let workload_api_js_service = host_client
        .write()
        .await
        .add_js_service(workload_stream_service)
        .await?;

    let hc_http_gw_storetore = spawn_hc_http_gw_watcher(
        host_id,
        Arc::clone(&host_client),
        Arc::clone(&workload_api_js_service),
        shutdown_rx.resubscribe(),
    )
    .await?;

    // Instantiate the Workload API
    let workload_api = HostJobApi {
        hc_http_gw_storetore,
    };

    // Created/ensured by the host agent inventory service (which is started prior to this service)
    let device_id = std::fs::read_to_string(MACHINE_ID_PATH).map_err(|e| {
        HostAgentError::service_failed(
            "workload service",
            &format!("reading device id from path: {}", e),
        )
    })?;

    workload_api_js_service
        .add_consumer(
            ServiceConsumerBuilder::new(
                "update_workload".to_string(),
                WorkloadServiceSubjects::Update,
                generate_service_call!(workload_api, update_workload, ApiOptions { device_id }),
            )
            .with_subject_prefix(host_id.to_lowercase())
            .with_response_subject_fn(create_callback_subject(
                WorkloadServiceSubjects::HandleStatusUpdate
                    .as_ref()
                    .to_string(),
            ))
            .into(),
        )
        .await?;

    workload_api_js_service
        .add_consumer(
            ServiceConsumerBuilder::new(
                "fetch_workload_status".to_string(),
                WorkloadServiceSubjects::SendStatus,
                generate_service_call!(workload_api, fetch_workload_status),
            )
            .with_subject_prefix(host_id.to_lowercase())
            .with_response_subject_fn(create_callback_subject(
                WorkloadServiceSubjects::HandleStatusUpdate
                    .as_ref()
                    .to_string(),
            ))
            .into(),
        )
        .await?;

    log::info!("Workload service setup complete, waiting for shutdown signal...");

    // Wait for shutdown signal
    // Keep the service running until shutdown signal is received
    // and pass the shutdown signal to the spawned HC HTTP GW watcher
    let _ = shutdown_rx.recv().await;
    log::info!("Shutdown signal received in workload service");

    log::info!("Workload service shutting down gracefully");
    Ok(())
}

async fn get_or_create_nats_kv_store(
    hc_http_gw_worker_kv_bucket_name: &str,
    nats_js_client: &mut JsClient,
) -> WorkloadOpResult<Store> {
    match nats_js_client
        .js_context
        .get_key_value(hc_http_gw_worker_kv_bucket_name)
        .await
    {
        Ok(kv) => Ok(kv),
        Err(e) => {
            match e.kind() {
                async_nats::jetstream::context::KeyValueErrorKind::GetBucket => {
                    // in this case the bucket has not been created yet.

                    let config = async_nats::jetstream::kv::Config {
                        bucket: hc_http_gw_worker_kv_bucket_name.to_string(),
                        // taken verbatim from the docstring. there is no use-case for keeping the history now, other than any unknown debugging scenarios that may come up
                        history: 10,
                        ..Default::default()
                    };

                    nats_js_client
                        .js_context
                        .create_key_value(config.clone())
                        .await
                        .map_err(|e| {
                            WorkloadError::jetstream_failed(&format!(
                                "creating KV with config {:?}: {}",
                                config, e
                            ))
                        })
                }
                async_nats::jetstream::context::KeyValueErrorKind::InvalidStoreName => {
                    Err(WorkloadError::kv_failed(&format!(
                        "invalid store name '{}', this is a bug.",
                        hc_http_gw_worker_kv_bucket_name
                    )))
                }
                async_nats::jetstream::context::KeyValueErrorKind::JetStream => Err(
                    WorkloadError::jetstream_failed("jetstream error, this is a bug."),
                ),
            }
        }
    }
}

// processes all existing and changed keys for the NATS bucket that's used to
// remember and communicate the need for HC HTTP GW consumers to.
async fn spawn_hc_http_gw_watcher(
    host_id: &str,
    nats_js_client: Arc<RwLock<JsClient>>,
    nats_js_service: Arc<JsStreamService>,
    shutdown_rx: broadcast::Receiver<()>,
) -> HostAgentResult<async_nats::jetstream::kv::Store> {
    const HC_HTTP_GW_WORKER_KV_BUCKET_NAME_PREFIX: &str = "HC-HTTP-GW-WORKER";
    let hc_http_gw_worker_kv_bucket_name =
        format!("{HC_HTTP_GW_WORKER_KV_BUCKET_NAME_PREFIX}_{host_id}");

    let http_gw_worker_kv_store = get_or_create_nats_kv_store(
        &hc_http_gw_worker_kv_bucket_name,
        &mut (*nats_js_client.write().await),
    )
    .await?;

    tokio::spawn({
        let http_gw_worker_kv_store = http_gw_worker_kv_store.clone();
        let mut shutdown_rx = shutdown_rx;
        async move {
            let mut final_stream = {
                let initial_entries_stream = http_gw_worker_kv_store
                    .keys()
                    .await?
                    .into_stream()
                    .filter_map(|try_key| async {
                        match try_key {
                            Ok(key) => Some((key, None)),
                            Err(e) => {
                                log::error!(
                                "error getting key from {hc_http_gw_worker_kv_bucket_name}: {e}"
                            );
                                None
                            }
                        }
                    })
                    .boxed();

                let watch_stream = http_gw_worker_kv_store
                    .watch_all()
                    .await
                    .map_err(|e| {
                        async_nats::Error::from(format!(
                            "Error wile watching all changes on {}: {}",
                            HC_HTTP_GW_WORKER_KV_BUCKET_NAME_PREFIX, e
                        ))
                    })?
                    .into_stream()
                    .filter_map(|watch| async {
                        match watch {
                            Ok(entry) => Some((entry.key.to_string(), Some(entry))),
                            Err(e) => {
                                log::warn!(
                                    "Error watching {hc_http_gw_worker_kv_bucket_name}: {e}"
                                );
                                None
                            }
                        }
                    })
                    .boxed();

                initial_entries_stream.chain(watch_stream)
            };

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        log::info!("Shutdown signal received in HC HTTP GW watcher");
                        break;
                    }
                    next = final_stream.next() => {
                        match next {
                            Some((key, maybe_entry)) => {
                                log::debug!("[hc_http_gw_worker] processing {key:?}");

                                let value = match maybe_entry {
                                    Some(entry) => entry.value,
                                    None => match http_gw_worker_kv_store
                                        .get(&key)
                                        .await
                                        .map_err(|e| async_nats::Error::from(format!("retrieving {} from {}: {}", key, hc_http_gw_worker_kv_bucket_name, e)))?
                                    {
                                        Some(value) => value,
                                        None => {
                                            log::warn!(
                                                "expected {key} to be in {hc_http_gw_worker_kv_bucket_name}"
                                            );
                                            continue;
                                        }
                                    },
                                };

                                let HcHttpGwWorkerKvBucketValue {
                                    desired_state,
                                    hc_http_gw_url_base,
                                    installed_app_id,
                                } = match serde_json::from_slice::<HcHttpGwWorkerKvBucketValue>(&value)
                                    .map_err(|e| format!("deserializing bytes for {}: {}", key, e)) {
                                    Ok(de) => de,
                                    Err(e) => {
                                        log::error!("Encountered error, skipping: {e}");
                                        continue;
                                    }
                                };

                                let subject_suffix = HcHttpGwRequest::nats_subject_suffix(&installed_app_id);
                                let consumer_name = sanitize_nats_name(&subject_suffix);

                                match desired_state {
                                    WorkloadStateDiscriminants::Running => {
                                        // Check if consumer already exists before creating
                                        match nats_js_service.get_consumer_stream_info(&consumer_name).await {
                                            Ok(_) => {
                                                log::debug!("Consumer {} already exists, skipping creation", consumer_name);
                                            }
                                            Err(_) => {
                                                // Consumer doesn't exist, create it
                                                match nats_js_service
                                                    .add_consumer(
                                                        ServiceConsumerBuilder::new(
                                                            consumer_name.clone(),
                                                            subject_suffix.to_string(),
                                                            {
                                                                let installed_app_id = installed_app_id.clone();
                                                                Arc::new(move |msg: Arc<async_nats::Message>| {
                                                                    let installed_app_id = installed_app_id.clone();
                                                                    let http_gw_url_base = hc_http_gw_url_base.clone();
                                                                    Box::pin(async move {
                                                                        let (_, response) = hc_http_gw_consumer_handler(
                                                                            msg,
                                                                            http_gw_url_base,
                                                                            installed_app_id,
                                                                        )
                                                                        .await
                                                                        .map_err(|e| ServiceError::Internal {
                                                                            message: e.to_string(),
                                                                            context: None,
                                                                        })?;

                                                                        let response_msg = HcHttpGwResponseMsg {
                                                                            response,
                                                                            response_subject: None,
                                                                        };

                                                                        Ok(response_msg)
                                                                    })
                                                                })
                                                            },
                                                        )
                                                        .with_response_subject_fn(Arc::new(
                                                            |tags: HashMap<String, String>| {
                                                                tags.keys().map(ToString::to_string).collect::<Vec<_>>()
                                                            },
                                                        ))
                                                        .into(),
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        match nats_js_service.get_consumer_stream_info(&consumer_name).await {
                                                            Ok(info) => log::debug!("Successfully created consumer {} with info: {:?}", consumer_name, info),
                                                            Err(e) => log::debug!("Successfully created consumer {} but failed to get info: {}", consumer_name, e),
                                                        }
                                                    },
                                                    Err(e) => log::error!("Failed to create consumer {}: {}", consumer_name, e),
                                                }
                                            }
                                        }
                                    }

                                    // in all other cases we want the consumer to be removed
                                    WorkloadStateDiscriminants::Reported
                                    | WorkloadStateDiscriminants::Assigned
                                    | WorkloadStateDiscriminants::Pending
                                    | WorkloadStateDiscriminants::Updated
                                    | WorkloadStateDiscriminants::Deleted
                                    | WorkloadStateDiscriminants::Uninstalled
                                    | WorkloadStateDiscriminants::Error
                                    | WorkloadStateDiscriminants::Unknown => {
                                        log::debug!("About to remove consumer {} for state {:?}..", consumer_name, desired_state);
                                        let delete_result = nats_js_service.delete_consumer(&consumer_name).await;
                                        if delete_result {
                                            log::debug!("Successfully removed consumer {}", consumer_name);
                                        } else {
                                            log::warn!("Failed to remove consumer {}", consumer_name);
                                        }
                                    }
                                }
                            }
                            None => {
                                log::warn!("HC HTTP GW watcher stream ended, will restart in 2 seconds");
                                // Check for shutdown signal during the restart delay
                                tokio::select! {
                                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
                                        break; // Break inner loop and restart stream
                                    }
                                    _ = shutdown_rx.recv() => {
                                        log::info!("Shutdown signal received during stream restart delay");
                                        return Ok::<(), async_nats::Error>(()); // Exit watcher task
                                    }
                                }
                            }
                        }
                    }
                }
            }

            log::info!("HC HTTP GW watcher shutting down gracefully");
            Ok(())
        }
    });

    Ok(http_gw_worker_kv_store)
}

const HTTP_CLIENT_TIMEOUT_SECS: u64 = 30;

async fn hc_http_gw_consumer_handler(
    msg: Arc<async_nats::Message>,
    http_gw_url_base: Url,
    workload_id: String,
) -> WorkloadOpResult<(HcHttpGwRequest, HcHttpGwResponse)> {
    let incoming_request_result = serde_json::from_slice::<HcHttpGwRequest>(&msg.payload);
    log::info!("Processing incoming request: {incoming_request_result:?}");

    let request = incoming_request_result?;

    let local_request_url_suffix = request
        .get_checked_url(Some(http_gw_url_base), &workload_id)
        .map_err(|e| {
            WorkloadError::workload_failed(&format!(
                "getting checked url path from request {:?}: {}",
                request, e
            ))
        })?;

    // Create HTTP client with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(HTTP_CLIENT_TIMEOUT_SECS))
        .build()?;

    let response = client
        .execute(reqwest::Request::new(Method::GET, local_request_url_suffix))
        .await?
        .error_for_status()?;

    let response_headers = response
        .headers()
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec().into_boxed_slice()))
        .collect();

    let response_bytes = response.bytes().await?;

    let response = HcHttpGwResponse {
        response_headers,
        response_bytes,
    };

    log::debug!("got response: {response:?}");

    Ok((request, response))
}
