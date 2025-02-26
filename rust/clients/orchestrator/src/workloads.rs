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
use std::str::FromStr;
use std::{path::PathBuf, sync::Arc, time::Duration};
use util_libs::nats::{
    jetstream_client::{self, JsClient},
    types::{ConsumerBuilder, Credentials, EndpointType, JsClientBuilder, JsServiceBuilder},
};
use workload::{
    orchestrator_api::OrchestratorWorkloadApi, types::WorkloadServiceSubjects, WorkloadServiceApi,
    WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "_ADMIN_INBOX.orchestrator";

pub async fn run(
    admin_creds_path: &Option<PathBuf>,
    nats_connect_timeout_secs: u64,
    db_client: MongoDBClient,
) -> Result<(), async_nats::Error> {
    // ==================== Setup NATS ====================
    let nats_url = jetstream_client::get_nats_url();
    let creds_path = admin_creds_path
        .to_owned()
        .ok_or(PathBuf::from_str(&jetstream_client::get_nats_creds_by_nsc(
            "HOLO", "ADMIN", "admin",
        )))
        .map(Credentials::Path)
        .map_err(|e| anyhow!("Failed to locate admin credential path. Err={:?}", e))?;

    let mut orchestrator_workload_client = tokio::select! {
        client = async {loop {
            let c = JsClient::new(JsClientBuilder {
                nats_url: nats_url.clone(),
                name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
                inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
                credentials: Some(vec![creds_path.clone()]),
                ping_interval: Some(Duration::from_secs(10)),
                request_timeout: Some(Duration::from_secs(5)),
                listeners: vec![jetstream_client::with_event_listeners(jetstream_client::get_event_listeners())],
            })
            .await
                .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"));

                match c {
                    Ok(client) => break client,
                    Err(e) => {
                        let duration = tokio::time::Duration::from_millis(100);
                        log::warn!("{}, retrying in {duration:?}", e);
                        tokio::time::sleep(duration).await;
                    }
                }
            }} => client,
        _ = {
            log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
        } => {
            return Err(async_nats::Error::from(anyhow!("timed out waiting for NATS on {:?}", nats_url)));
        }
    };

    // ==================== Setup JS Stream Service ====================
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
    orchestrator_workload_client
        .add_js_service(workload_stream_service)
        .await?;

    let workload_service = orchestrator_workload_client
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

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_workload_client.close().await?;
    Ok(())
}
