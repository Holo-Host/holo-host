/*
 This client is associated with the:
- WORKLOAD account
- hpos user

// This client is responsible for:
  - subscribing to workload streams
    - installing new workloads
    - removing workloads
    - send workload status upon request
  - sending active periodic workload reports
*/

use super::endpoints;
use anyhow::Result;
use std::time::Duration;
use util_libs::js_microservice::JsStreamService;
use util_libs::nats_client::{self, Client as NatClient, EndpointType, EventListener};
use workload::{
    WorkloadApi, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ, WORKLOAD_SRV_VERSION,
};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_CLIENT_INBOX_PREFIX: &str = "_host_inbox";

// TODO: Use _user_creds_path for auth once we add in the more resilient auth pattern.
pub async fn run(_user_creds_path: &str) -> Result<(), async_nats::Error> {
    log::info!("HPOS Agent Client: Connecting to server...");
    // Connect to Nats server
    let nats_url = nats_client::get_nats_url();
    let event_listeners = get_host_workload_event_listeners();

    let host_workload_client =
        nats_client::DefaultClient::new(nats_client::NewDefaultClientParams {
            nats_url,
            name: HOST_AGENT_CLIENT_NAME.to_string(),
            inbox_prefix: format!(
                "{}_{}",
                HOST_AGENT_CLIENT_INBOX_PREFIX, "host_id_placeholder"
            ),
            credentials_path: None, // TEMP: Some(user_creds_path),
            opts: vec![nats_client::with_event_listeners(event_listeners)],
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
        })
        .await?;

    // Call workload service and call relevant endpoints
    let js_context = JsStreamService::get_context(host_workload_client.client.clone());
    let workload_stream = JsStreamService::new(
        js_context,
        WORKLOAD_SRV_NAME,
        WORKLOAD_SRV_DESC,
        WORKLOAD_SRV_VERSION,
        WORKLOAD_SRV_SUBJ,
    )
    .await?;

    let workload_api = WorkloadApi::new().await?;

    // Register Workload Streams for Host Agent to consume
    // (subjects should be published by orchestrator or nats-db-connector)
    workload_stream
        .add_local_consumer(
            "start_workload",
            "start",
            EndpointType::Async(endpoints::start_workload(&workload_api).await),
            None,
        )
        .await?;

    workload_stream
        .add_local_consumer(
            "signal_status_update",
            "signal_status_update",
            EndpointType::Async(endpoints::signal_status_update(&workload_api).await),
            None,
        )
        .await?;

    workload_stream
        .add_local_consumer(
            "remove_workload",
            "remove",
            EndpointType::Async(endpoints::remove_workload(&workload_api).await),
            None,
        )
        .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;
    log::warn!("CTRL+C detected. Please press CTRL+C again within 5 seconds to confirm exit...");
    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => log::warn!("Resuming service."),
        _ = tokio::signal::ctrl_c() => log::error!("Shutting down."),
    }

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    host_workload_client.close().await?;

    Ok(())
}

pub fn get_host_workload_event_listeners() -> Vec<EventListener> {
    // TODO: Use duration in handlers..
    let published_msg_handler = |msg: &str, _duration: Duration| {
        log::info!(
            "Successfully published message for {} client: {:?}",
            HOST_AGENT_CLIENT_NAME,
            msg
        );
    };
    let failure_handler = |err: &str, _duration: Duration| {
        log::error!(
            "Failed to publish message for {} client: {:?}",
            HOST_AGENT_CLIENT_NAME,
            err
        );
    };

    let event_listeners = vec![
        nats_client::on_msg_published_event(published_msg_handler),
        nats_client::on_msg_failed_event(failure_handler),
    ];

    event_listeners
}
