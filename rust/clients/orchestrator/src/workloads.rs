/*
 This client is associated with the:
- WORKLOAD account
- orchestrator user

// This client is responsible for:
*/

use anyhow::{anyhow, Result};
use std::{collections::HashMap, sync::Arc, time::Duration};
use async_nats::Message;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use workload::{
    WorkloadServiceApi, orchestrator_api::OrchestratorWorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION, types::{WorkloadServiceSubjects, ApiResult}
};
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::{JsServiceParamsPartial, ResponseSubjectsGenerator},
    nats_js_client::{self, EndpointType, JsClient, NewJsClientParams},
};

const ORCHESTRATOR_WORKLOAD_CLIENT_NAME: &str = "Orchestrator Workload Agent";
const ORCHESTRATOR_WORKLOAD_CLIENT_INBOX_PREFIX: &str = "_orchestrator_workload_inbox";

pub fn create_callback_subject_to_host(is_prefix: bool, tag_name: String, sub_subject_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |tag_map: HashMap<String, String>| -> Vec<String> {
        if is_prefix {
            let matching_tags = tag_map.into_iter().fold(vec![], |mut acc, (k, v)| {
                if k.starts_with(&tag_name) {
                    acc.push(v)
                }
                acc
            });
            return matching_tags;
        } else if let Some(tag) = tag_map.get(&tag_name) {
            return vec![format!("{}.{}", tag, sub_subject_name)];
        }
        log::error!("WORKLOAD Error: Failed to find {}. Unable to send orchestrator response to hosting agent for subject {}. Fwding response to `WORKLOAD.ERROR.INBOX`.", tag_name, sub_subject_name);
        vec!["WORKLOAD.ERROR.INBOX".to_string()]
    })
}

pub async fn run() -> Result<(), async_nats::Error> {
    // ==================== Setup NATS ====================
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

    // ==================== Setup DB ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;
    
    // ==================== Setup API & Register Endpoints ====================
    // Instantiate the Workload API (requires access to db client)
    let workload_api = OrchestratorWorkloadApi::new(&client).await?;

    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects below are published by external Developer, the Nats-DB-Connector, or the Host Agent
    let workload_add_subject = serde_json::to_string(&WorkloadServiceSubjects::Add)?;
    let workload_update_subject = serde_json::to_string(&WorkloadServiceSubjects::Update)?;
    let workload_remove_subject = serde_json::to_string(&WorkloadServiceSubjects::Remove)?;
    let workload_db_insert_subject = serde_json::to_string(&WorkloadServiceSubjects::Insert)?;
    let workload_db_modification_subject = serde_json::to_string(&WorkloadServiceSubjects::Modify)?;
    let workload_handle_status_subject = serde_json::to_string(&WorkloadServiceSubjects::HandleStatusUpdate)?;
    let workload_start_subject = serde_json::to_string(&WorkloadServiceSubjects::Start)?;
    let workload_handle_update_subject = serde_json::to_string(&WorkloadServiceSubjects::HandleUpdate)?;

    let workload_service = orchestrator_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Workload Service. Unable to spin up Orchestrator Workload Client."
        ))?;

    // Published by Developer
    workload_service
        .add_consumer::<ApiResult>(
            "add_workload", // consumer name
             &workload_add_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(|api: OrchestratorWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.add_workload(msg).await
                }
            })),
            None,
        )
        .await?;

        workload_service
        .add_consumer::<ApiResult>(
            "update_workload", // consumer name
             &workload_update_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(|api: OrchestratorWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.update_workload(msg).await
                }
            })),
            None,
        )
        .await?;

    
    workload_service
        .add_consumer::<ApiResult>(
            "remove_workload", // consumer name
             &workload_remove_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(|api: OrchestratorWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.remove_workload(msg).await
                }
            })),
            None,
        )
        .await?;
    
    // Automatically published by the Nats-DB-Connector
    workload_service
        .add_consumer::<ApiResult>(
            "handle_db_insertion", // consumer name
             &workload_db_insert_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(|api: OrchestratorWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.handle_db_insertion(msg).await
                }
            })),
            Some(create_callback_subject_to_host(true, "assigned_hosts".to_string(), workload_start_subject)),
        )
        .await?;

    workload_service
        .add_consumer::<ApiResult>(
            "handle_db_modification", // consumer name
                &workload_db_modification_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(|api: OrchestratorWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.handle_db_modification(msg).await
                }
            })),
            Some(create_callback_subject_to_host(true, "assigned_hosts".to_string(), workload_handle_update_subject)),
        )
        .await?;

    // Published by the Host Agent
    workload_service
    .add_consumer::<ApiResult>(
        "handle_status_update", // consumer name
        &workload_handle_status_subject, // consumer stream subj
        EndpointType::Async(workload_api.call(|api: OrchestratorWorkloadApi, msg: Arc<Message>| {
            async move {
                api.handle_status_update(msg).await
            }
        })),
        None,
    )
    .await?;

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_workload_client.close().await?;
    Ok(())
}
