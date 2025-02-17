/*
This client is associated with the:
    - ADMIN account
    - admin user

This client is responsible for:
    - initalizing connection and handling interface with db
    - registering with the host worklload service to:
        - handling requests to add workloads
        - handling requests to update workloads
        - handling requests to remove workloads
        - handling workload status updates
    - interfacing with mongodb DB
    - keeping service running until explicitly cancelled out
*/

use anyhow::Result;
use async_nats::Message;
use mongodb::Client as MongoDBClient;
use std::vec;
use std::{collections::HashMap, sync::Arc};
use util_libs::{
    js_stream_service::{JsServiceParamsPartial, ResponseSubjectsGenerator},
    nats_js_client::{EndpointType, JsClient},
};
use workload::{
    orchestrator_api::OrchestratorWorkloadApi,
    types::{WorkloadApiResult, WorkloadServiceSubjects},
    WorkloadServiceApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ,
    WORKLOAD_SRV_VERSION,
};

pub fn create_callback_subject_to_host(
    is_prefix: bool,
    tag_name: String,
    sub_subject_name: String,
) -> ResponseSubjectsGenerator {
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

pub async fn run(nats_client: JsClient, db_client: MongoDBClient) -> Result<(), async_nats::Error> {
    // ==================== Setup NATS ====================
    // Setup JS Stream Service
    let service_config = JsServiceParamsPartial {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };
    let workload_service = nats_client.add_js_service(service_config).await?;

    // ==================== Setup API & Register Endpoints ====================
    // Instantiate the Workload API (requires access to db client)
    let workload_api = OrchestratorWorkloadApi::new(&db_client).await?;

    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects are published by external Developer (via external api), the Nats-DB-Connector, or the Hosting Agent
    let workload_add_subject = serde_json::to_string(&WorkloadServiceSubjects::Add)?;
    let workload_update_subject = serde_json::to_string(&WorkloadServiceSubjects::Update)?;
    let workload_remove_subject = serde_json::to_string(&WorkloadServiceSubjects::Remove)?;
    let workload_db_insert_subject = serde_json::to_string(&WorkloadServiceSubjects::Insert)?;
    let workload_db_modification_subject = serde_json::to_string(&WorkloadServiceSubjects::Modify)?;
    let workload_handle_status_subject =
        serde_json::to_string(&WorkloadServiceSubjects::HandleStatusUpdate)?;
    let workload_install_subject = serde_json::to_string(&WorkloadServiceSubjects::Install)?;
    let workload_update_installed_subject =
        serde_json::to_string(&WorkloadServiceSubjects::UpdateInstalled)?;

    // Published by Developer
    workload_service
        .add_consumer::<WorkloadApiResult>(
            "add_workload",        // consumer name
            &workload_add_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.add_workload(msg).await
                },
            )),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "update_workload",        // consumer name
            &workload_update_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.update_workload(msg).await
                },
            )),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "remove_workload",        // consumer name
            &workload_remove_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.remove_workload(msg).await
                },
            )),
            None,
        )
        .await?;

    // Automatically published by the Nats-DB-Connector
    workload_service
        .add_consumer::<WorkloadApiResult>(
            "handle_db_insertion",       // consumer name
            &workload_db_insert_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.handle_db_insertion(msg).await
                },
            )),
            Some(create_callback_subject_to_host(
                true,
                "assigned_hosts".to_string(),
                workload_install_subject,
            )),
        )
        .await?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "handle_db_modification",          // consumer name
            &workload_db_modification_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.handle_db_modification(msg).await
                },
            )),
            Some(create_callback_subject_to_host(
                true,
                "assigned_hosts".to_string(),
                workload_update_installed_subject,
            )),
        )
        .await?;

    // Published by the Host Agent
    workload_service
        .add_consumer::<WorkloadApiResult>(
            "handle_status_update",          // consumer name
            &workload_handle_status_subject, // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.handle_status_update(msg).await
                },
            )),
            None,
        )
        .await?;

    Ok(())
}
