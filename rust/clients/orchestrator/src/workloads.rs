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

use super::utils;
use anyhow::{anyhow, Result};
use async_nats::Message;
use mongodb::Client as MongoDBClient;
use std::sync::Arc;
use util_libs::nats::{
    jetstream_client::JsClient,
    types::{ConsumerBuilder, EndpointType, JsServiceBuilder},
};
use workload::{
    orchestrator_api::OrchestratorWorkloadApi, types::WorkloadServiceSubjects, WorkloadServiceApi,
    WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

pub async fn run(
    mut orchestrator_client: JsClient,
    db_client: MongoDBClient,
) -> Result<JsClient, async_nats::Error> {
    // ==================== Setup JS Stream Service ====================
    // Instantiate the Workload API (requires access to db client)
    let workload_api = OrchestratorWorkloadApi::new(&db_client).await?;

    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };
    orchestrator_client
        .add_js_service(workload_stream_service)
        .await?;

    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects are published by external Developer (via external api), the Nats-DB-Connector, or the Hosting Agent
    let workload_service = orchestrator_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Workload Service. Unable to spin up Orchestrator Workload Service."
        ))?;

    // Published by Developer
    workload_service
        .add_consumer(ConsumerBuilder {
            name: "add_workload".to_string(),
            endpoint_subject: WorkloadServiceSubjects::Add.as_ref().to_string(),
            handler: EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.add_workload(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "update_workload".to_string(),
            endpoint_subject: WorkloadServiceSubjects::Update.as_ref().to_string(),
            handler: EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.update_workload(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "remove_workload".to_string(),
            endpoint_subject: WorkloadServiceSubjects::Remove.as_ref().to_string(),
            handler: EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.remove_workload(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    // Automatically published by the Nats-DB-Connector
    workload_service
        .add_consumer(ConsumerBuilder {
            name: "handle_db_insertion".to_string(),
            endpoint_subject: WorkloadServiceSubjects::Insert.as_ref().to_string(),
            handler: EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.handle_db_insertion(msg).await
                },
            )),
            response_subject_fn: Some(utils::create_callback_subject_to_host(
                true,
                "assigned_hosts".to_string(),
                WorkloadServiceSubjects::Install.as_ref().to_string(),
            )),
        })
        .await?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "handle_db_modification".to_string(),
            endpoint_subject: WorkloadServiceSubjects::Modify.as_ref().to_string(),
            handler: EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.handle_db_modification(msg).await
                },
            )),
            response_subject_fn: Some(utils::create_callback_subject_to_host(
                true,
                "assigned_hosts".to_string(),
                WorkloadServiceSubjects::UpdateInstalled
                    .as_ref()
                    .to_string(),
            )),
        })
        .await?;

    // Published by the Host Agent
    workload_service
        .add_consumer(ConsumerBuilder {
            name: "handle_status_update".to_string(),
            endpoint_subject: WorkloadServiceSubjects::HandleStatusUpdate
                .as_ref()
                .to_string(),
            handler: EndpointType::Async(workload_api.call(
                |api: OrchestratorWorkloadApi, msg: Arc<Message>| async move {
                    api.handle_status_update(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    Ok(orchestrator_client)
}
