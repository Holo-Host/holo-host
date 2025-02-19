/*
 This client is associated with the:
    - WORKLOAD account
    - host user

This client is responsible for subscribing to workload streams that handle:
    - installing new workloads onto the hosting device
    - removing workloads from the hosting device
    - sending workload status upon request
    - sending out active periodic workload reports
*/

use anyhow::{anyhow, Result};
use async_nats::Message;
use std::{path::PathBuf, sync::Arc, time::Duration};
use util_libs::{
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{self, EndpointType},
};
use workload::{
    host_api::HostWorkloadApi,
    types::{WorkloadApiResult, WorkloadServiceSubjects},
    WorkloadServiceApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ,
    WORKLOAD_SRV_VERSION,
};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_WORKLOAD_INBOX";

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(
    host_pubkey: &str,
    host_creds_path: &Option<PathBuf>,
) -> Result<nats_js_client::JsClient, async_nats::Error> {
    log::info!("Host Agent Client: Connecting to server...");
    log::info!("host_creds_path : {:?}", host_creds_path);
    log::info!("host_pubkey : {}", host_pubkey);

    let pubkey_lowercase = host_pubkey.to_string().to_lowercase();

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
    let host_workload_client = nats_js_client::JsClient::new(nats_js_client::NewJsClientParams {
        nats_url: nats_url.clone(),
        name: HOST_AGENT_CLIENT_NAME.to_string(),
        inbox_prefix: format!("{}_{}", HOST_AGENT_INBOX_PREFIX, pubkey_lowercase),
        service_params: vec![workload_stream_service_params.clone()],
        credentials_path: host_creds_path
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(29)),
        listeners: vec![nats_js_client::with_event_listeners(
            event_listeners.clone(),
        )],
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"))?;

    // ==================== Setup API & Register Endpoints ====================
    // Instantiate the Workload API
    let workload_api = HostWorkloadApi::default();

    // Register Workload Streams for Host Agent to consume and process
    // NB: Subjects are published by orchestrator
    let workload_install_subject = serde_json::to_string(&WorkloadServiceSubjects::Install)?;
    let workload_send_status_subject = serde_json::to_string(&WorkloadServiceSubjects::SendStatus)?;
    let workload_uninstall_subject = serde_json::to_string(&WorkloadServiceSubjects::Uninstall)?;
    let workload_update_installed_subject =
        serde_json::to_string(&WorkloadServiceSubjects::UpdateInstalled)?;

    let workload_service = host_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to spin up Host Agent."
        ))?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "install_workload",                                            // consumer name
            &format!("{}.{}", pubkey_lowercase, workload_install_subject), // consumer stream subj
            EndpointType::Async(
                workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.install_workload(msg).await
                }),
            ),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "update_installed_workload", // consumer name
            &format!("{}.{}", pubkey_lowercase, workload_update_installed_subject), // consumer stream subj
            EndpointType::Async(
                workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.update_workload(msg).await
                }),
            ),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "uninstall_workload",                                            // consumer name
            &format!("{}.{}", pubkey_lowercase, workload_uninstall_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.uninstall_workload(msg).await
                },
            )),
            None,
        )
        .await?;

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "send_workload_status", // consumer name
            &format!("{}.{}", pubkey_lowercase, workload_send_status_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.send_workload_status(msg).await
                },
            )),
            None,
        )
        .await?;

    Ok(host_workload_client)
}
