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

use super::utils::create_callback_subject;
use anyhow::Result;
use nats_utils::{
    generate_service_call,
    jetstream_client::JsClient,
    types::{JsServiceBuilder, ServiceConsumerBuilder},
};
use tokio::sync::RwLock;
use workload::{
    host_api::HostWorkloadApi, types::WorkloadServiceSubjects, WORKLOAD_SRV_DESC,
    WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

// TODO: Use _host_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(mut host_client: JsClient, host_id: &str) -> Result<JsClient, async_nats::Error> {
    log::info!("Host Agent Client: starting workload service...");
    log::info!("host_id : {}", host_id);

    // Register Workload Streams for Host Agent to consume
    // NB: Subjects are published by orchestrator or nats-db-connector
    let workload_stream_service = JsServiceBuilder {
        name: WORKLOAD_SRV_NAME.to_string(),
        description: WORKLOAD_SRV_DESC.to_string(),
        version: WORKLOAD_SRV_VERSION.to_string(),
        service_subject: WORKLOAD_SRV_SUBJ.to_string(),
    };

    // Instantiate the Workload API
    let workload_api = HostWorkloadApi {
        worload_api_js_service: std::sync::Arc::new(RwLock::new(
            host_client.add_js_service(workload_stream_service).await?,
        )),
    };

    workload_api
        .worload_api_js_service
        .write()
        .await
        .add_consumer(
            ServiceConsumerBuilder::new(
                "update_workload".to_string(),
                WorkloadServiceSubjects::Update,
                generate_service_call!(workload_api, update_workload),
            )
            .with_subject_prefix(host_id.to_lowercase())
            .with_response_subject_fn(create_callback_subject(
                WorkloadServiceSubjects::HandleStatusUpdate
                    .as_ref()
                    .to_string(),
            ))
            .into(),
        )
        .await?;

    workload_api
        .worload_api_js_service
        .write()
        .await
        .add_consumer(
            ServiceConsumerBuilder::new(
                "fetch_workload_status".to_string(),
                WorkloadServiceSubjects::SendStatus,
                generate_service_call!(workload_api, fetch_workload_status),
            )
            .with_subject_prefix(host_id.to_lowercase())
            .with_response_subject_fn(create_callback_subject(
                WorkloadServiceSubjects::HandleStatusUpdate
                    .as_ref()
                    .to_string(),
            ))
            .into(),
        )
        .await?;

    Ok(host_client)
}
