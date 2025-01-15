/*
 This client is associated with the:
- WORKLOAD account
- orchestrator user

// This client is responsible for:
*/

use anyhow::{anyhow, Result};
use std::{sync::Arc, time::Duration};
use async_nats::Message;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use workload::{
    WorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{self, EndpointType, JsClient, NewJsClientParams},
};

const ORCHESTRATOR_WORKLOAD_CLIENT_NAME: &str = "Orchestrator Workload Agent";
const ORCHESTRATOR_WORKLOAD_CLIENT_INBOX_PREFIX: &str = "_orchestrator_workload_inbox";

pub async fn run() -> Result<(), async_nats::Error> {
    // ==================== NATS Setup ====================
    let nats_url = nats_js_client::get_nats_url();
    let creds_path = nats_js_client::get_nats_client_creds("HOLO", "WORKLOAD", "orchestrator");
    let event_listeners = nats_js_client::get_event_listeners();

    // Setup JS Stream Service
    let workload_stream_service_params = JsServiceParamsPartial {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    let orchestrator_workload_client =
        JsClient::new(NewJsClientParams {
            nats_url,
            name: ORCHESTRATOR_WORKLOAD_CLIENT_NAME.to_string(),
            inbox_prefix: ORCHESTRATOR_WORKLOAD_CLIENT_INBOX_PREFIX.to_string(),
            service_params: vec![workload_stream_service_params],
            credentials_path: Some(creds_path),
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
    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects below are published by external Developer, the Nats-DB-Connector, or the Host Agent
    let workload_service = orchestrator_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Workload Service. Unable to spin up Orchestrator Workload Client."
        ))?;

    // Published by Developer
    workload_service
        .add_local_consumer::<workload::types::ApiResult>(
            "add_workload",
            "add",
            EndpointType::Async(workload_api.call(|api: WorkloadApi, msg: Arc<Message>| {
                async move {
                    api.add_workload(msg).await
                }
            })),
            None,
        )
        .await?;

    // Automatically published by the Nats-DB-Connector
    workload_service
        .add_local_consumer::<workload::types::ApiResult>(
            "handle_db_insertion",
            "insert",
            EndpointType::Async(workload_api.call(|api: WorkloadApi, msg: Arc<Message>| {
                async move {
                    api.handle_db_insertion(msg).await
                }
            })),
            None,
        )
        .await?;
    
    // Published by the Host Agent
    workload_service
    .add_local_consumer::<workload::types::ApiResult>(
        "handle_status_update",
        "read_status_update",
        EndpointType::Async(workload_api.call(|api: WorkloadApi, msg: Arc<Message>| {
            async move {
                api.handle_status_update(msg).await
            }
        })),
        None,
    )
    .await?;


    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_workload_client.close().await?;
    Ok(())
}
