use super::utils::create_callback_subject;
use anyhow::{Context, Result};
use hpos_hal::inventory::MACHINE_ID_PATH;
use hpos_updates::{
    host_api::HostUpdatesApi, types::HostUpdateServiceSubjects, HPOS_UPDATES_SVC_DESC,
    HPOS_UPDATES_SVC_NAME, HPOS_UPDATES_SVC_SUBJ, HPOS_UPDATES_SVC_VERSION,
};
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ServiceConsumerBuilder},
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run(
    host_client: Arc<RwLock<JsClient>>,
    host_id: &str,
) -> Result<(), async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_id : {}", host_id);

    // Note: `MACHINE_ID_PATH` is created by the host agent inventory service
    let device_id =
        std::fs::read_to_string(MACHINE_ID_PATH).context("reading device id from path")?;

    let host_updates_stream_service = JsServiceBuilder {
        name: HPOS_UPDATES_SVC_NAME.to_string(),
        description: HPOS_UPDATES_SVC_DESC.to_string(),
        version: HPOS_UPDATES_SVC_VERSION.to_string(),
        service_subject: HPOS_UPDATES_SVC_SUBJ.to_string(),
        maybe_source_js_domain: None,
    };

    let host_updates_client = host_client
        .write()
        .await
        .add_js_service(host_updates_stream_service)
        .await?;

    let host_updates_api = HostUpdatesApi {};

    // Request nixos update on host agent
    host_updates_client
        .add_consumer(
            ServiceConsumerBuilder::new(
                HostUpdateServiceSubjects::Update.as_ref().to_string(), // this sets the subject that will invoke the `handle_host_update` fn
                HPOS_UPDATES_SVC_SUBJ,
                generate_service_call!(host_updates_api, handle_host_update_command),
            )
            .with_subject_prefix(device_id)
            .with_response_subject_fn(create_callback_subject(
                HostUpdateServiceSubjects::Status.as_ref().to_string(),
            ))
            .into(),
        )
        .await?;

    Ok(())
}
