use super::utils::create_callback_subject_to_host;
use anyhow::Result;
use hpos_updates::{
    orchestrator_api::OrchestratorHostUpdatesApi, types::HostUpdateServiceSubjects,
    HOST_UPDATES_SRV_DESC, HOST_UPDATES_SRV_NAME, HOST_UPDATES_SRV_SUBJ, HOST_UPDATES_SRV_VERSION,
    HOST_UPDATES_SUBJECT, ORCHESTRATOR_SUBJECT_PREFIX, TAG_MAP_PREFIX_DESIGNATED_HOST,
};
use mongodb::Client as MongoDBClient;
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ServiceConsumerBuilder},
};
use std::sync::Arc;

pub async fn run(
    mut nats_client: JsClient,
    db_client: MongoDBClient,
) -> Result<(), async_nats::Error> {
    // Setup JS Stream Service
    let host_updates_stream_service = JsServiceBuilder {
        name: HOST_UPDATES_SRV_NAME.to_string(),
        description: HOST_UPDATES_SRV_DESC.to_string(),
        version: HOST_UPDATES_SRV_VERSION.to_string(),
        service_subject: HOST_UPDATES_SRV_SUBJ.to_string(),
        maybe_source_js_domain: None,
    };
    let host_updates_client = nats_client
        .add_js_service(host_updates_stream_service)
        .await?;

    // Instantiate the Host Updates API (requires access to db client)
    let host_updates_api = Arc::new(OrchestratorHostUpdatesApi::new(&db_client).await?);

    // Request nixos update on host agent
    host_updates_client
        .add_consumer(
            ServiceConsumerBuilder::new(
                HostUpdateServiceSubjects::Update.as_ref().to_string(), // this sets the subject that will invoke the `handle_host_update` fn
                HOST_UPDATES_SUBJECT,
                generate_service_call!(host_updates_api, handle_host_update),
            )
            .with_subject_prefix(ORCHESTRATOR_SUBJECT_PREFIX.to_string())
            .with_response_subject_fn(create_callback_subject_to_host(
                true,
                TAG_MAP_PREFIX_DESIGNATED_HOST.to_string(),
                HostUpdateServiceSubjects::Update.as_ref().to_string(), // this references the subject to which the host agent has subscribed (note: it will be prefixed by the device_id)
            ))
            .into(),
        )
        .await?;

    // Handle nixos update response from host agent
    host_updates_client
        .add_consumer(
            ServiceConsumerBuilder::new(
                HostUpdateServiceSubjects::Status.as_ref().to_string(), // this sets the subject that will invoke the `handle_host_update` fn (prefixed by orchestrator)
                HOST_UPDATES_SUBJECT,
                generate_service_call!(host_updates_api, handle_host_update_response),
            )
            .with_subject_prefix(ORCHESTRATOR_SUBJECT_PREFIX.to_string())
            .into(),
        )
        .await?;

    Ok(())
}
