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

use super::utils::{add_workload_consumer, create_callback_subject_to_host};
use anyhow::Result;
use mongodb::Client as MongoDBClient;
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ServiceConsumerBuilder},
};
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

    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects are published by external Developer (via external api), the Nats-DB-Connector, or the Hosting Agent
    let workload_service = orchestrator_client
        .add_js_service(workload_stream_service)
        .await?;

    // Subjects published by Developer:
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "add_workload".to_string(),
            WorkloadServiceSubjects::Add,
            generate_service_call!(workload_api, add_workload),
        ),
        &workload_service,
    )
    .await?;

    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "update_workload".to_string(),
            WorkloadServiceSubjects::Update,
            generate_service_call!(workload_api, update_workload),
        ),
        &workload_service,
    )
    .await?;

    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "delete_workload".to_string(),
            WorkloadServiceSubjects::Delete,
            generate_service_call!(workload_api, delete_workload),
        ),
        &workload_service,
    )
    .await?;

    // Subjects published by the Nats-DB-Connector:
    let db_insertion_response_handler = create_callback_subject_to_host(
        true,
        "assigned_hosts".to_string(),
        WorkloadServiceSubjects::Install.as_ref().to_string(),
    );
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "handle_db_insertion".to_string(),
            WorkloadServiceSubjects::Insert,
            generate_service_call!(workload_api, handle_db_insertion),
        )
        .with_response_subject_fn(db_insertion_response_handler),
        &workload_service,
    )
    .await?;

    let db_modification_response_handler = create_callback_subject_to_host(
        true,
        "assigned_hosts".to_string(),
        WorkloadServiceSubjects::Update.as_ref().to_string(),
    );
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "handle_db_modification".to_string(),
            WorkloadServiceSubjects::Modify,
            generate_service_call!(workload_api, handle_db_modification),
        )
        .with_response_subject_fn(db_modification_response_handler),
        &workload_service,
    )
    .await?;

    // Subjects published by the Host Agent:
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "handle_status_update".to_string(),
            WorkloadServiceSubjects::HandleStatusUpdate,
            generate_service_call!(workload_api, handle_status_update),
        ),
        &workload_service,
    )
    .await?;

    Ok(orchestrator_client)
}
