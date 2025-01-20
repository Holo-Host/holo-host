/*
 This client is associated with the:
- WORKLOAD account
- hpos user

// This client is responsible for subscribing to workload streams that handle:
    - installing new workloads onto the hosting device
    - removing workloads from the hosting device
    - sending workload status upon request
    - sending out active periodic workload reports
*/

use anyhow::{anyhow, Result};
use std::{sync::Arc, time::Duration};
use async_nats::Message;
use util_libs::{
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{self, EndpointType},
};
use workload::{
    WorkloadServiceApi, host_api::HostWorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
    types::{WorkloadServiceSubjects, ApiResult}
};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_host_inbox";

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(host_pubkey: &str, host_creds_path: &str) -> Result<(), async_nats::Error> {
    log::info!("HPOS Agent Client: Connecting to server...");
    log::info!("host_creds_path : {}", host_creds_path);
    log::info!("host_pubkey : {}", host_pubkey);

    // ==================== Setup NATS ====================
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
    let host_workload_client =
        nats_js_client::JsClient::new(nats_js_client::NewJsClientParams {
            nats_url,
            name: HOST_AGENT_CLIENT_NAME.to_string(),
            inbox_prefix: format!(
                "{}_{}",
                HOST_AGENT_INBOX_PREFIX, host_pubkey
            ),
            service_params: vec![workload_stream_service_params],
            credentials_path: Some(host_creds_path.to_string()),
            opts: vec![nats_js_client::with_event_listeners(event_listeners)],
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
        })
        .await?;    

    // ==================== Setup API & Register Endpoints ====================
    // Instantiate the Workload API
    let workload_api = HostWorkloadApi::default();
    
    // Register Workload Streams for Host Agent to consume and process
    // NB: Subjects are published by orchestrator
    let workload_start_subject = serde_json::to_string(&WorkloadServiceSubjects::Start)?;
    let workload_send_status_subject = serde_json::to_string(&WorkloadServiceSubjects::SendStatus)?;
    let workload_uninstall_subject = serde_json::to_string(&WorkloadServiceSubjects::Uninstall)?;
    let workload_update_installed_subject = serde_json::to_string(&WorkloadServiceSubjects::UpdateInstalled)?;

    let workload_service = host_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to spin up Host Agent."
        ))?;

    workload_service
        .add_consumer::<ApiResult>(
            "start_workload", // consumer name
            &format!("{}.{}", host_pubkey, workload_start_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.start_workload(msg).await
                }
            })),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<ApiResult>(
            "update_installed_workload", // consumer name
            &format!("{}.{}", host_pubkey, workload_update_installed_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.update_workload(msg).await
                }
            })),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<ApiResult>(
            "uninstall_workload", // consumer name
            &format!("{}.{}", host_pubkey, workload_uninstall_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.uninstall_workload(msg).await
                }
            })),
            None,
        )
        .await?;

        workload_service
        .add_consumer::<ApiResult>(
            "send_workload_status", // consumer name
            &format!("{}.{}", host_pubkey, workload_send_status_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| {
                async move {
                    api.send_workload_status(msg).await
                }
            })),
            None,
        )
        .await?;

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    host_workload_client.close().await?;
    Ok(())
}
