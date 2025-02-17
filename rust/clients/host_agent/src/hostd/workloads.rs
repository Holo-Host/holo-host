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

use anyhow::Result;
use async_nats::Message;
use std::sync::Arc;
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

pub async fn run(
    host_client: nats_js_client::JsClient,
    host_pubkey: &str,
) -> Result<(), async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_pubkey : {}", host_pubkey);

    // ==================== Setup NATS Service ====================
    // Setup JS Stream Service
    let workload_service_config = JsServiceParamsPartial {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    let workload_service = host_client.add_js_service(workload_service_config).await?;

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

    workload_service
        .add_consumer::<WorkloadApiResult>(
            "install_workload",                                       // consumer name
            &format!("{}.{}", host_pubkey, workload_install_subject), // consumer stream subj
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
            &format!("{}.{}", host_pubkey, workload_update_installed_subject), // consumer stream subj
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
            "uninstall_workload",                                       // consumer name
            &format!("{}.{}", host_pubkey, workload_uninstall_subject), // consumer stream subj
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
            "send_workload_status",                                       // consumer name
            &format!("{}.{}", host_pubkey, workload_send_status_subject), // consumer stream subj
            EndpointType::Async(workload_api.call(
                |api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.send_workload_status(msg).await
                },
            )),
            None,
        )
        .await?;

    Ok(())
}
