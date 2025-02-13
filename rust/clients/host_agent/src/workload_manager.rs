/*
 This client is associated with the:
- WORKLOAD account
- hpos user

// This client is responsible for:
  - subscribing to workload streams
    - installing new workloads
    - removing workloads
    - sending workload status upon request
    - sending active periodic workload reports
*/

use anyhow::{anyhow, Result};
use async_nats::Message;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::{path::PathBuf, sync::Arc, time::Duration};
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{self, EndpointType},
};
use workload::{
    WorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_host_inbox";

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(
    host_pubkey: &str,
    host_creds_path: &Option<PathBuf>,
) -> Result<nats_js_client::JsClient, async_nats::Error> {
    log::info!("HPOS Agent Client: Connecting to server...");
    log::info!("host_creds_path : {:?}", host_creds_path);
    log::info!("host_pubkey : {}", host_pubkey);

    // ==================== NATS Setup ====================
    // Connect to Nats server
    let nats_url = nats_js_client::get_nats_url();
    log::info!("nats_url : {}", nats_url);

    let event_listeners = nats_js_client::get_event_listeners();

    // Setup JS Stream Service
    let workload_stream_service_params = JsServiceParamsPartial {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    // Spin up Nats Client and loaded in the Js Stream Service
    let host_workload_client = nats_js_client::JsClient::new(nats_js_client::NewJsClientParams {
        nats_url: nats_url.clone(),
        name: HOST_AGENT_CLIENT_NAME.to_string(),
        inbox_prefix: format!("{}_{}", HOST_AGENT_INBOX_PREFIX, host_pubkey),
        service_params: vec![workload_stream_service_params.clone()],
        credentials_path: host_creds_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        opts: vec![nats_js_client::with_event_listeners(
            event_listeners.clone(),
        )],
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(29)),
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"))?;

    // ==================== DB Setup ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;

    // Generate the Workload API with access to db
    let workload_api = WorkloadApi::new(&client).await?;

    // ==================== API ENDPOINTS ====================
    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let workload_service = host_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to spin up Host Agent."
        ))?;

    workload_service
        .add_local_consumer::<workload::types::ApiResult>(
            "start_workload",
            "start",
            EndpointType::Async(workload_api.call(
                |api: WorkloadApi, msg: Arc<Message>| async move { api.start_workload(msg).await },
            )),
            None,
        )
        .await?;

    workload_service
        .add_local_consumer::<workload::types::ApiResult>(
            "send_workload_status",
            "send_status",
            EndpointType::Async(
                workload_api.call(|api: WorkloadApi, msg: Arc<Message>| async move {
                    api.send_workload_status(msg).await
                }),
            ),
            None,
        )
        .await?;

    workload_service
        .add_local_consumer::<workload::types::ApiResult>(
            "uninstall_workload",
            "uninstall",
            EndpointType::Async(
                workload_api.call(|api: WorkloadApi, msg: Arc<Message>| async move {
                    api.uninstall_workload(msg).await
                }),
            ),
            None,
        )
        .await?;

    Ok(host_workload_client)
}
