/*
 This client is associated with the:
- WORKLOAD account
- hpos user

// This client is responsible for:
  - subscribing to workload streams
    - installing new workloads
    - removing workloads
    - send workload status upon request
  - sending active periodic workload reports
*/

use super::endpoints;
use anyhow::{anyhow, Result};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::time::Duration;
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::{JsServiceParamsPartial, JsStreamService},
    nats_js_client::{self, EndpointType, EventListener, JsClient},
};
use workload::{
    WorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_CLIENT_INBOX_PREFIX: &str = "_host_inbox";

// TODO: Use _user_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(user_creds_path: &str) -> Result<(), async_nats::Error> {
    log::info!("HPOS Agent Client: Connecting to server...");

    // ==================== NATS Setup ====================
    // Connect to Nats server
    let nats_url = nats_js_client::get_nats_url();
    let event_listeners = nats_js_client::get_event_listeners();

    // Setup JS Stream Service
    let workload_stream_service_params = JsServiceParamsPartial {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    // Spin up Nats Client and loaded in the Js Stream Service
    let host_workload_client =
        nats_js_client::DefaultJsClient::new(nats_js_client::NewDefaultJsClientParams {
            nats_url,
            name: HOST_AGENT_CLIENT_NAME.to_string(),
            inbox_prefix: format!(
                "{}_{}",
                HOST_AGENT_CLIENT_INBOX_PREFIX, "host_id_placeholder"
            ),
            service_params: vec![workload_stream_service_params],
            credentials_path: Some(user_creds_path.to_string()),
            opts: vec![nats_js_client::with_event_listeners(event_listeners)],
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
        })
        .await?;

    // ==================== DB Setup ====================

    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;

    // Generate the Workload API with access to db
    let workload_api = WorkloadApi::new(&client).await?;

    // ==================== API ENDPOINTS ====================
    // Register Workload Streams for Host Agent to consume
    // (subjects should be published by orchestrator or nats-db-connector)
    let workload_service = host_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to spin up Host Agent."
        ))?;

    workload_service
        .add_local_consumer(
            "start_workload",
            "start",
            EndpointType::Async(endpoints::start_workload(&workload_api).await),
            None,
        )
        .await?;

    workload_service
        .add_local_consumer(
            "signal_status_update",
            "signal_status_update",
            EndpointType::Async(endpoints::signal_status_update(&workload_api).await),
            None,
        )
        .await?;

    workload_service
        .add_local_consumer(
            "remove_workload",
            "remove",
            EndpointType::Async(endpoints::remove_workload(&workload_api).await),
            None,
        )
        .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;
    log::warn!("CTRL+C detected. Please press CTRL+C again within 5 seconds to confirm exit...");
    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => log::warn!("Resuming service."),
        _ = tokio::signal::ctrl_c() => log::error!("Shutting down."),
    }

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    host_workload_client.close().await?;

    Ok(())
}
