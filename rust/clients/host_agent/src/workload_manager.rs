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
use std::sync::Arc;
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{EndpointType, JsClient},
};
use workload::{
    WorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(mut host_client: JsClient) -> Result<(), async_nats::Error> {
    log::info!("HPOS Agent Client: Connecting to server...");
    // ==================== NATS Setup ====================
    // Setup JS Stream Service
    let workload_stream_service = JsServiceParamsPartial {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    host_client.add_js_service(workload_stream_service).await?;
    let workload_service = host_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to start service. Unable to fetch workload service."
        ))?;

    // ==================== DB Setup ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let db_client_options = ClientOptions::parse(mongo_uri).await?;
    let db_client = MongoDBClient::with_options(db_client_options)?;

    // Generate the Workload API with access to db
    let workload_api = WorkloadApi::new(&db_client).await?;

    // ==================== API ENDPOINTS ====================
    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator
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

    Ok(())
}
