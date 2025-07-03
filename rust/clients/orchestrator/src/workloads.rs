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

use std::sync::Arc;

use super::utils::{add_workload_consumer, create_callback_subject_to_host};
use anyhow::Result;
use mongodb::Client as MongoDBClient;
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ServiceConsumerBuilder, ServiceError},
};
use std::time::Duration;
use workload::{
    orchestrator_api::OrchestratorWorkloadApi, types::WorkloadServiceSubjects,
    TAG_MAP_PREFIX_ASSIGNED_HOST, WORKLOAD_ORCHESTRATOR_SUBJECT_PREFIX, WORKLOAD_SRV_DESC,
    WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

pub async fn run(
    mut orchestrator_client: JsClient,
    db_client: MongoDBClient,
) -> Result<JsClient, async_nats::Error> {
    // Instantiate the Workload API (requires access to db client)
    let workload_api = Arc::new(OrchestratorWorkloadApi::new(&db_client).await?);

    // Register Workload Streams for Orchestrator to consume and process
    // NB: These subjects are published by external Developer (via external api), the Nats-DB-Connector, or the Hosting Agent
    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
        maybe_source_js_domain: None,
    };

    // Register Workload Streams for Orchestrator to consume and proceess
    // NB: These subjects are published by external Developer (via external api), the Nats-DB-Connector, or the Hosting Agent
    let workload_service = orchestrator_client
        .add_js_service(workload_stream_service)
        .await?;

    /*
       TODO(design): sort out the holo-public-API <> orchestrator interaction

       Add, Update, Delete are currently not expected to be sent by the holo-public-API.
       instead the holo-public-API creates DB entries and _something_ external to the orchestrator,
       e.g. the [mongodb-nats-connector](https://github.com/damianiandrea/mongodb-nats-connector) sends messages upon picking up changes from mongodb.
    */

    /* TODO(clarify requirements): currently a sequence of add, delete, add (with the same id) leads to a duplicate key error:

        Mar 18 16:12:20 dev-orch holo-orchestrator-start[322]: [2025-03-18T15:12:20Z DEBUG db_utils::mongodb] Inserting new document
        Mar 18 16:12:20 dev-orch holo-orchestrator-start[322]: [2025-03-18T15:12:20Z ERROR db_utils::mongodb] MongoDB insert_one_into operation failed: Kind: An error occurred when trying to execute a write operation: WriteError(WriteError { code: 11000, code_name: None, message: "E11000 duplicate key error collection: holo-hosting.workload index: _id_ dup key: { _id: ObjectId('67d2ef2a67d4b619a54286c4') }", details: None }), labels: {}
        Mar 18 16:12:20 dev-orch holo-orchestrator-start[322]: [2025-03-18T15:12:20Z ERROR workload] Failed to process workload request. Subject=WORKLOAD.add, Payload=Workload { _id: Some(ObjectId("67d2ef2a67d4b619a54286c4")), metadata: Metadata { is_deleted: false, deleted_at: None, updated_at: None, created_at: None }, assigned_developer: ObjectId("67d98d547999125b12b5cac9"), version: "", min_hosts: 0, system_specs: SystemSpecs { capacity: Capacity { drive: 1, cores: 1 }, avg_network_speed: 0, avg_uptime: 0.0 }, assigned_hosts: [], status: WorkloadStatus { id: Some(ObjectId("67d2ef2a67d4b619a54286c4")), desired: Installed, actual: Unknown("most uncertain") }, manifest: HolochainDhtV1(WorkloadManifestHolochainDhtV1 { happ_binary_url: Url { scheme: "https", cannot_be_a_base: false, username: "", password: None, host: Some(Domain("gist.github.com")), port: None, path: "/steveej/5443d6d15395aa23081f1ee04712b2b3/raw/fdacb9b723ba83743567f2a39a8bfbbffb46b1f0/test-zome.bundle", query: None, fragment: None }, network_seed: "just-testing", memproof: None, bootstrap_server_urls: None, sbd_server_urls: None, holochain_feature_flags: None, holochain_version: None }) }: Database error: Kind: An error occurred when trying to execute a write operation: WriteError(WriteError { code: 11000, code_name: None, message: "E11000 duplicate key error collection: holo-hosting.workload index: _id_ dup key: { _id: ObjectId('67d2ef2a67d4b619a54286c4') }", details: None }), labels: {}
    */

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

    // Subjects published `remote_cmd` cli (internal tool)
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "manage_workload_on_host".to_string(),
            WorkloadServiceSubjects::Insert,
            generate_service_call!(workload_api, manage_workload_on_host),
        )
        .with_response_subject_fn(create_callback_subject_to_host(
            true,
            TAG_MAP_PREFIX_ASSIGNED_HOST.to_string(),
            WorkloadServiceSubjects::Update.to_string(),
        )),
        &workload_service,
    )
    .await?;

    // Subjects published by the Host Agent:
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "handle_status_update".to_string(),
            WorkloadServiceSubjects::HandleStatusUpdate,
            generate_service_call!(workload_api, handle_status_update),
        )
        .with_subject_prefix(WORKLOAD_ORCHESTRATOR_SUBJECT_PREFIX.to_string()),
        &workload_service,
    )
    .await?;

    // Start workload collection stream in background
    let workload_api_clone = workload_api.clone();
    let js_context_clone = orchestrator_client.js_context.clone();
    tokio::spawn(async move {
        const MAX_RETRY_DELAY: Duration = Duration::from_secs(300); // 300 sec = 5 min
        const MAX_RETRIES: i32 = 10;

        let mut retry_count = 0;
        let mut retry_delay = Duration::from_secs(1);

        loop {
            match workload_api_clone
                .stream_workload_changes(
                    js_context_clone.clone(),
                    WORKLOAD_SRV_SUBJ.to_string(),
                    create_callback_subject_to_host(
                        true,
                        TAG_MAP_PREFIX_ASSIGNED_HOST.to_string(),
                        WorkloadServiceSubjects::Update.to_string(),
                    ),
                )
                .await
            {
                Ok(_) => {
                    // Stream completed successfully (shouldn't happen normally)
                    log::warn!("Workload collection stream completed unexpectedly");
                    break;
                }
                Err(e) => {
                    // Check if this is a permanent error that shouldn't be re-tried
                    if is_permanent_error(&e) {
                        log::error!("Request error found in workload collection stream. Skipping retry. err={}", e);
                        break;
                    }

                    retry_count += 1;
                    if retry_count > MAX_RETRIES {
                        log::error!(
                            "Workload collection stream failed after {} retries. Last error: {}",
                            MAX_RETRIES,
                            e
                        );
                        break;
                    }

                    log::warn!(
                        "Workload collection stream error: {}. Retrying in {:?} (attempt {}/{})",
                        e,
                        retry_delay,
                        retry_count,
                        MAX_RETRIES
                    );

                    tokio::time::sleep(retry_delay).await;
                    retry_delay = std::cmp::min(retry_delay * 2, MAX_RETRY_DELAY);
                }
            }
        }
    });

    Ok(orchestrator_client)
}

// Helper function to determine if an error is permanent and shouldn't be retried
// Add specific error types that should not be retried below
// eg: request error
fn is_permanent_error(e: &ServiceError) -> bool {
    matches!(e, ServiceError::Request { .. })
}
