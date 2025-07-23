use std::sync::Arc;

use anyhow::Result;
use hpos_updates::{
    HostUpdatesServiceApi, OrchestratorHostUpdatesApi, HOST_UPDATES_SRV_DESC,
    HOST_UPDATES_SRV_NAME, HOST_UPDATES_SRV_SUBJ, HOST_UPDATES_SRV_VERSION, HOST_UPDATES_SUBJECT,
    TAG_MAP_PREFIX_ASSIGNED_HOST,
};
use mongodb::Client as MongoDBClient;
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ResponseSubjectsGenerator, ServiceConsumerBuilder},
};
use std::collections::HashMap;

pub fn create_callback_subject(sub_subject_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |_tag_map: HashMap<String, String>| -> Vec<String> {
        vec![format!(
            "{WORKLOAD_ORCHESTRATOR_SUBJECT_PREFIX}.{sub_subject_name}",
        )]
    })
}

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

    host_updates_client
        .add_consumer(
            ServiceConsumerBuilder::new(
                "handle_host_update_response".to_string(),
                HOST_UPDATES_SUBJECT,
                generate_service_call!(host_updates_api, handle_host_update_response),
            )
            .with_subject_prefix("*".to_string())
            .into(),
        )
        .await?;

    // Subjects published by hosting agent:
    host_updates_client
        .add_consumer(
            ServiceConsumerBuilder::new(
                "update_host_channel".to_string(),
                HOST_UPDATES_SUBJECT,
                generate_service_call!(host_updates_api, handle_host_update),
            )
            .with_response_subject_fn(create_callback_subject_to_host(
                true,
                TAG_MAP_PREFIX_ASSIGNED_HOST.to_string(),
                "handle_host_update".to_string(), // this is the subject the host agent will subcribe / listen to
            )),
        )
        .await?;

    Ok(())
}
