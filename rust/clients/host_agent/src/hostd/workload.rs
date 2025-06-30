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

use super::utils::create_callback_subject;
use anyhow::{Context, Result};
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
use tokio::sync::RwLock;
use url::Url;
use workload::{
    host_api::{HcHttpGwWorkerKvBucketValue, HostWorkloadApi},
    types::WorkloadServiceSubjects,
    WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

pub async fn run(
    host_client: Arc<RwLock<JsClient>>,
    host_id: &str,
) -> Result<(), async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_id : {}", host_id);

    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    let worload_api_js_service = host_client
        .write()
        .await
        .add_js_service(workload_stream_service)
        .await?;

    let hc_http_gw_storetore = spawn_hc_http_gw_watcher(
        host_id,
        Arc::clone(&host_client),
        Arc::clone(&worload_api_js_service),
    )
    .await?;

    // Instantiate the Workload API
    let workload_api = HostWorkloadApi {
        hc_http_gw_storetore,
    };

    // Created/ensured by the host agent inventory service (which is started prior to this service)
    let device_id =
        std::fs::read_to_string(MACHINE_ID_PATH).context("reading device id from path")?;

    worload_api_js_service
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

    worload_api_js_service
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

    Ok(())
}

async fn get_or_create_nats_kv_store(
    hc_http_gw_worker_kv_bucket_name: &str,
    nats_js_client: &mut JsClient,
) -> anyhow::Result<Store> {
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
                        .create_key_value(config)
                        .await
                        .context("creating KV with config {config:?}")
                }
                async_nats::jetstream::context::KeyValueErrorKind::InvalidStoreName => {
                    anyhow::bail!(
                        "invalid store name '{hc_http_gw_worker_kv_bucket_name}', this is a bug."
                    )
                }
                async_nats::jetstream::context::KeyValueErrorKind::JetStream => {
                    anyhow::bail!("jetstream error, this is a bug.")
                }
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
) -> anyhow::Result<async_nats::jetstream::kv::Store> {
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
                    .context("watching all changes on {HC_HTTP_GW_WORKER_KV_BUCKET_NAME_PREFIX}")?
                    .into_stream()
                    .filter_map(|watch| async {
                        match watch {
                            Ok(entry) => Some((entry.key.to_string(), Some(entry))),
                            Err(e) => {
                                log::warn!(
                                    "error watching {hc_http_gw_worker_kv_bucket_name}: {e}"
                                );
                                None
                            }
                        }
                    })
                    .boxed();

                initial_entries_stream.chain(watch_stream)
            };

            while let Some((key, maybe_entry)) = final_stream.next().await {
                log::debug!("[hc_http_gw_worker] processing {key:?}");

                let value = match maybe_entry {
                    Some(entry) => entry.value,
                    None => match http_gw_worker_kv_store
                        .get(&key)
                        .await
                        .context("retrieving {key} from {{hc_http_gw_worker_kv_bucket_name}}")?
                    {
                        Some(value) => value,
                        None => {
                            log::warn!(
                                "expected {key} to be in {{hc_http_gw_worker_kv_bucket_name}}"
                            );
                            continue;
                        }
                    },
                };

                let HcHttpGwWorkerKvBucketValue {
                    desired_state,
                    hc_http_gw_url_base,
                    installed_app_id,
                } = match serde_json::from_slice(&value).context("deserializing bytes for {key}") {
                    Ok(de) => de,
                    Err(e) => {
                        log::error!("error countered, skipping: {e}");
                        continue;
                    }
                };

                let subject_suffix = HcHttpGwRequest::nats_subject_suffix(&installed_app_id);
                let consumer_name = sanitize_nats_name(&subject_suffix);

                match desired_state {
                    WorkloadStateDiscriminants::Running => {
                        // TODO(correctness): what if the consumer already exists?
                        let consumer = nats_js_service
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
                                                    Arc::clone(&msg),
                                                    http_gw_url_base.clone(),
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
                            .await;

                        log::debug!(
                            "consumer there? {:?}. add result: {consumer:?}",
                            nats_js_service
                                .get_consumer_stream_info(&consumer_name)
                                .await
                        );
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
                        let removed = nats_js_service.delete_consumer(&consumer_name).await;
                        log::debug!("removing consumer {consumer_name}: {removed}");
                    }
                }
            }

            anyhow::Result::<(), anyhow::Error>::Err(anyhow::anyhow!(
                "no flow should lead here (yet, until we've implemented graceful shutdown)."
            ))
        }
    });

    Ok(http_gw_worker_kv_store)
}

async fn hc_http_gw_consumer_handler(
    msg: Arc<async_nats::Message>,
    http_gw_url_base: Url,
    workload_id: String,
) -> anyhow::Result<(HcHttpGwRequest, HcHttpGwResponse)> {
    let incoming_request_result = serde_json::from_slice::<HcHttpGwRequest>(&msg.payload);
    log::debug!("incoming message with request: {incoming_request_result:?}");

    let request = incoming_request_result?;

    let local_request_url_suffix = request
        .get_checked_url(Some(http_gw_url_base), &workload_id)
        .context(format!("getting checked url path from request {request:?}"))?;

    let response = reqwest::Client::new()
        .execute(reqwest::Request::new(Method::GET, local_request_url_suffix))
        .await
        .context("executing request")?
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
