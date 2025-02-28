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
use std::{path::PathBuf, sync::Arc, time::Duration};
use util_libs::nats::{
    jetstream_client,
    types::{ConsumerBuilder, Credentials, EndpointType, JsClientBuilder, JsServiceBuilder},
};
use workload::{
    host_api::HostWorkloadApi, types::WorkloadServiceSubjects, WorkloadServiceApi,
    WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};
const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_HPOS_INBOX";

pub async fn run(
    host_pubkey: &str,
    host_creds_path: &Option<PathBuf>,
) -> Result<jetstream_client::JsClient, async_nats::Error> {
    log::info!("Host Agent Client: Connecting to server...");
    log::info!("host_creds_path : {:?}", host_creds_path);
    log::info!("host_pubkey : {}", host_pubkey);

    let pubkey_lowercase = host_pubkey.to_string().to_lowercase();

    // ==================== Setup NATS ====================
    // Connect to Nats server
    let nats_url = jetstream_client::get_nats_url();
    log::info!("nats_url : {}", nats_url);

    let host_creds = host_creds_path
        .to_owned()
        .map(Credentials::Path)
        .ok_or_else(|| async_nats::Error::from("error"))?;

    // Spin up Nats Client and load the Js Stream Service
    let mut host_workload_client = jetstream_client::JsClient::new(JsClientBuilder {
        nats_url: nats_url.clone(),
        name: HOST_AGENT_CLIENT_NAME.to_string(),
        inbox_prefix: format!("{}.{}", HOST_AGENT_INBOX_PREFIX, pubkey_lowercase),
        credentials: Some(vec![host_creds.clone()]),
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(29)),
        listeners: vec![jetstream_client::with_event_listeners(
            jetstream_client::get_event_listeners(),
        )],
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"))?;

    // ==================== Setup JS Stream Service ====================
    // Instantiate the Workload API
    let workload_api = HostWorkloadApi::default();

    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };
    host_workload_client
        .add_js_service(workload_stream_service)
        .await?;

    let workload_service = host_workload_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to run holo agent workload service."
        ))?;

    workload_service
        .add_consumer(ConsumerBuilder {
            name: "install_workload".to_string(),
            subject: format!(
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
            subject: format!(
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
            subject: format!(
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
            subject: format!(
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

    Ok(host_workload_client)
}
