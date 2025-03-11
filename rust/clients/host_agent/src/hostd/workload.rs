/*
  This client is associated with the:
    - HPOS account
    - host user

  This client does not publish to any workload subjects.

  This client is responsible for subscribing to workload streams that handle:
    - installing new workloads onto the hosting device
    - removing workloads from the hosting device
    - sending workload status upon request
    - sending out active periodic workload reports
*/

use super::utils::{add_workload_consumer, create_callback_subject_to_orchestrator};
use anyhow::{anyhow, Result};
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ServiceConsumerBuilder},
};
use workload::{
    host_api::HostWorkloadApi, types::WorkloadServiceSubjects, WORKLOAD_SRV_DESC,
    WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

pub async fn run(
    mut host_client: JsClient,
    host_pubkey: &str,
) -> Result<JsClient, async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_pubkey : {}", host_pubkey);

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
    host_client.add_js_service(workload_stream_service).await?;

    let workload_service = host_client
        .get_js_service(WORKLOAD_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate workload service. Unable to run holo agent workload service."
        ))?;

    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "install_workload".to_string(),
            WorkloadServiceSubjects::Install,
            generate_service_call!(workload_api, install_workload),
        )
        .with_subject_prefix(host_pubkey.to_lowercase()),
        workload_service,
    )
    .await?;

    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "update_installed_workload".to_string(),
            WorkloadServiceSubjects::UpdateInstalled,
            generate_service_call!(workload_api, update_workload),
        )
        .with_subject_prefix(host_pubkey.to_lowercase()),
        workload_service,
    )
    .await?;

    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "uninstall_workload".to_string(),
            WorkloadServiceSubjects::Uninstall,
            generate_service_call!(workload_api, uninstall_workload),
        )
        .with_subject_prefix(host_pubkey.to_lowercase()),
        workload_service,
    )
    .await?;

    let update_workload_status_response = create_callback_subject_to_orchestrator(
        WorkloadServiceSubjects::HandleStatusUpdate
            .as_ref()
            .to_string(),
    );
    add_workload_consumer(
        ServiceConsumerBuilder::new(
            "fetch_workload_status".to_string(),
            WorkloadServiceSubjects::SendStatus,
            generate_service_call!(workload_api, fetch_workload_status),
        )
        .with_subject_prefix(host_pubkey.to_lowercase())
        .with_response_subject_fn(update_workload_status_response),
        workload_service,
    )
    .await?;

    Ok(host_client)
}
