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
        sanitization::sanitize_nats_name,
        JsServiceBuilder, ServiceConsumerBuilder, ServiceError,
    },
};
use reqwest::Method;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use url::Url;
use hpos_updates::{
    types::HostUpdateServiceSubjects, host_api::HostUpdatesApi,
    HOST_UPDATES_SRV_DESC, HOST_UPDATES_SRV_NAME, HOST_UPDATES_SRV_SUBJ, HOST_UPDATES_SRV_VERSION,
    ORCHESTRATOR_SUBJECT_PREFIX, TAG_MAP_PREFIX_DESIGNATED_HOST,
};

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(
    host_client: Arc<RwLock<JsClient>>,
    host_id: &str,
    jetstream_domain: &str,
) -> Result<(), async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_id : {}", host_id);

    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let host_updates_stream_service = JsServiceBuilder {
        name: HOST_UPDATES_SRV_NAME.to_string(),
        description: HOST_UPDATES_SRV_DESC.to_string(),
        version: HOST_UPDATES_SRV_VERSION.to_string(),
        service_subject: HOST_UPDATES_SRV_SUBJ.to_string(),
        maybe_source_js_domain: None,
    };

    let host_updates_client = host_client
        .write()
        .await
        .add_js_service(host_updates_stream_service)
        .await?;
    
    let host_updates_api = HostUpdatesApi { };

    // Created/ensured by the host agent inventory service (which is started prior to this service)
    let device_id =
        std::fs::read_to_string(MACHINE_ID_PATH).context("reading device id from path")?;

    // Request nixos update on host agent
    host_updates_client
        .add_consumer(
            ServiceConsumerBuilder::new(
                HostUpdateServiceSubjects::Update.as_ref().to_string(), // this sets the subject that will invoke the `handle_host_update` fn
                HOST_UPDATES_SRV_SUBJ,
                generate_service_call!(host_updates_api, handle_host_update_command),
            )
            .with_subject_prefix(device_id)
            .with_response_subject_fn(create_callback_subject(
                HostUpdateServiceSubjects::Status.as_ref().to_string()
            ))
            .into(),
        )
        .await?;

    Ok(())
}
