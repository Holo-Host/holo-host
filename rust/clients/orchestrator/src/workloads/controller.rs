/*
 This client is associated with the:
- WORKLOAD account
- orchestrator user

// This client is responsible for:
*/

use super::endpoints;
use anyhow::Result;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use tokio::time::Duration;
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsStreamService,
    nats_js_client::{self, EventListener},
};
use workload::{
    WorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

const ORCHESTRATOR_WORKLOAD_CLIENT_NAME: &str = "Orchestrator Workload Agent";
const ORCHESTRATOR_WORKLOAD_CLIENT_INBOX_PREFIX: &str = "_orchestrator_workload_inbox";

pub async fn run() -> Result<(), async_nats::Error> {
    // ==================== NATS Setup ====================
    let nats_url = nats_js_client::get_nats_url();
    let creds_path = nats_js_client::get_nats_client_creds("HOLO", "WORKLOAD", "orchestrator");
    let event_listeners = nats_js_client::get_event_listeners();

    let workload_service =
        nats_js_client::DefaultJsClient::new(nats_js_client::NewDefaultJsClientParams {
            nats_url,
            name: ORCHESTRATOR_WORKLOAD_CLIENT_NAME.to_string(),
            inbox_prefix: ORCHESTRATOR_WORKLOAD_CLIENT_INBOX_PREFIX.to_string(),
            opts: vec![nats_js_client::with_event_listeners(event_listeners)],
            credentials_path: Some(creds_path),
            ..Default::default()
        })
        .await?;

    // Create a new Jetstream Microservice
    let js_service = JsStreamService::new(
        workload_service.js.clone(),
        WORKLOAD_SRV_NAME,
        WORKLOAD_SRV_DESC,
        WORKLOAD_SRV_VERSION,
        WORKLOAD_SRV_SUBJ,
    )
    .await?;

    // ==================== DB Setup ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;

    // Generate the Workload API with access to db
    let workload_api = WorkloadApi::new(&client).await?;

    // ==================== API ENDPOINTS ====================

    // For ORCHESTRATOR to consume
    // (subjects should be published by developer)
    js_service
        .add_local_consumer(
            "add_workload",
            "add",
            nats_js_client::EndpointType::Async(endpoints::add_workload(workload_api).await),
            None,
        )
        .await?;

    js_service
        .add_local_consumer(
            "handle_changed_db_workload",
            "handle_change",
            nats_js_client::EndpointType::Async(endpoints::handle_db_change().await),
            None,
        )
        .await?;

    log::trace!(
        "{} Service is running. Waiting for requests...",
        WORKLOAD_SRV_NAME
    );

    Ok(())
}
