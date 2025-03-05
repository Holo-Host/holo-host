/*
This client is associated with the:
    - ADMIN account
    - admin user

This client is responsible for:
    - initalizing connection and handling interface with db
    - registering with the host workload service to:
        - handling requests to add workloads
        - handling requests to update workloads
        - handling requests to delete workloads
        - handling workload status updates
    - interfacing with mongodb DB
    - keeping service running until explicitly cancelled out
*/

use super::utils::{create_callback_subject_to_host, create_consumer, WorkloadConsumerBuilder};
use crate::generate_call_method;
use anyhow::{anyhow, Result};
use async_nats::Message;
use mongodb::Client as MongoDBClient;
use nats_utils::{jetstream_client::JsClient, types::JsServiceBuilder};
use std::sync::Arc;
use workload::{
    orchestrator_api::OrchestratorWorkloadApi, types::WorkloadServiceSubjects, WORKLOAD_SRV_DESC,
    WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

pub async fn run(
    mut orchestrator_client: JsClient,
    db_client: MongoDBClient,
) -> Result<JsClient, async_nats::Error> {
    // Instantiate the Workload API (requires access to db client)
    let workload_api = OrchestratorWorkloadApi::new(&db_client).await?;

    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects are published by external Developer (via external api), the Nats-DB-Connector, or the Hosting Agent
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

    // Subjects published by Developer:
    workload_service
        .add_consumer(create_consumer(WorkloadConsumerBuilder {
            name: "add_workload".to_string(),
            subject: WorkloadServiceSubjects::Add,
            async_handler: generate_call_method!(workload_api, add_workload),
            response_subject_fn: None,
        }))
        .await?;

    workload_service
        .add_consumer(create_consumer(WorkloadConsumerBuilder {
            name: "update_workload".to_string(),
            subject: WorkloadServiceSubjects::Update,
            async_handler: generate_call_method!(workload_api, update_workload),
            response_subject_fn: None,
        }))
        .await?;

    workload_service
        .add_consumer(create_consumer(WorkloadConsumerBuilder {
            name: "delete_workload".to_string(),
            subject: WorkloadServiceSubjects::Delete,
            async_handler: generate_call_method!(workload_api, delete_workload),
            response_subject_fn: None,
        }))
        .await?;

    // Subjects published by the Nats-DB-Connector:
    let db_insertion_response_handler = create_callback_subject_to_host(
        true,
        "assigned_hosts".to_string(),
        WorkloadServiceSubjects::Install.as_ref().to_string(),
    );
    workload_service
        .add_consumer(create_consumer(WorkloadConsumerBuilder {
            name: "handle_db_insertion".to_string(),
            subject: WorkloadServiceSubjects::Insert,
            async_handler: generate_call_method!(workload_api, handle_db_insertion),
            response_subject_fn: Some(db_insertion_response_handler),
        }))
        .await?;

    let db_modification_response_handler = create_callback_subject_to_host(
        true,
        "assigned_hosts".to_string(),
        WorkloadServiceSubjects::UpdateInstalled
            .as_ref()
            .to_string(),
    );
    workload_service
        .add_consumer(create_consumer(WorkloadConsumerBuilder {
            name: "handle_db_modification".to_string(),
            subject: WorkloadServiceSubjects::Modify,
            async_handler: generate_call_method!(workload_api, handle_db_modification),
            response_subject_fn: Some(db_modification_response_handler),
        }))
        .await?;

    // Subjects published by the Host Agent:
    workload_service
        .add_consumer(create_consumer(WorkloadConsumerBuilder {
            name: "handle_status_update".to_string(),
            subject: WorkloadServiceSubjects::HandleStatusUpdate,
            async_handler: generate_call_method!(workload_api, handle_status_update),
            response_subject_fn: None,
        }))
        .await?;

    Ok(orchestrator_client)
}
