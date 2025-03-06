/*
 This client is associated with the:
    - HPOS account
    - host user

This client is responsible for subscribing to workload streams that handle:
    - installing new workloads onto the hosting device
    - removing workloads from the hosting device
    - sending workload status upon request
    - sending out active periodic workload reports
*/

use anyhow::{anyhow, Result};
use async_nats::Message;
use std::sync::Arc;
use util_libs::nats::{
    jetstream_client::JsClient,
    types::{ConsumerBuilder, EndpointType, JsServiceBuilder},
};
use workload::{
    host_api::HostWorkloadApi, types::WorkloadServiceSubjects, WorkloadServiceApi,
    WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(
    mut host_client: JsClient,
    host_pubkey: &str,
) -> Result<JsClient, async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_pubkey : {}", host_pubkey);

    // Instantiate the Workload API
    let workload_api = HostWorkloadApi::default();
    let pubkey_lowercase = host_pubkey.to_string().to_lowercase();

    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };
    host_client.add_js_service(workload_stream_service).await?;

    let workload_service = host_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to run holo agent workload service."
        ))?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "install_workload".to_string(),
            endpoint_subject: format!(
                "{}.{}",
                pubkey_lowercase,
                WorkloadServiceSubjects::Install.as_ref()
            ),
            handler: EndpointType::Async(
                workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.install_workload(msg).await
                }),
            ),
            response_subject_fn: None,
        })
        .await?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "update_installed_workload".to_string(),
            endpoint_subject: format!(
                "{}.{}",
                pubkey_lowercase,
                WorkloadServiceSubjects::UpdateInstalled.as_ref()
            ),
            handler: EndpointType::Async(
                workload_api.call(|api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.update_workload(msg).await
                }),
            ),
            response_subject_fn: None,
        })
        .await?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "uninstall_workload".to_string(),
            endpoint_subject: format!(
                "{}.{}",
                pubkey_lowercase,
                WorkloadServiceSubjects::Uninstall.as_ref()
            ),
            handler: EndpointType::Async(workload_api.call(
                |api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.uninstall_workload(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "send_workload_status".to_string(),
            endpoint_subject: format!(
                "{}.{}",
                pubkey_lowercase,
                WorkloadServiceSubjects::SendStatus.as_ref()
            ),
            handler: EndpointType::Async(workload_api.call(
                |api: HostWorkloadApi, msg: Arc<Message>| async move {
                    api.send_workload_status(msg).await
                },
            )),
            response_subject_fn: None,
        })
        .await?;

    Ok(host_client)
}
