/*
This client is associated with the:
  - HPOS account
  - host user

This client is responsible for subscribing the host agent to workload stream endpoints:
  - installing new workloads
  - removing workloads
  - sending active periodic workload reports
  - sending workload status upon request
*/

use nats_utils::{
    jetstream_client::{get_event_listeners, with_event_listeners, JsClient},
    types::{JsClientBuilder, NatsRemoteArgs},
};
use std::time::Duration;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};
use crate::local_cmds::host::types::agent_client::{
    HostClient, HostClientConfig, TypeSpecificArgs,
};
use async_trait::async_trait;

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_HPOS_INBOX";

#[derive(Debug)]
pub struct HostAgentClient {
    pub client: JsClient,
}

#[async_trait]
impl HostClient for HostAgentClient {
    type Output = Self;

    async fn start(config: &HostClientConfig) -> HostAgentResult<Self::Output> {
        let client_args = match &config.type_args {
            TypeSpecificArgs::HostAgent(host_args) => host_args,
            _ => {
                return Err(HostAgentError::validation(
                    "Invalid client type for host agent client",
                ))
            }
        };

        let nats_url = &client_args.nats_url;
        let device_id_lowercase = config.device_id.to_lowercase();

        log::debug!("nats url : {nats_url:?}");
        log::debug!("device id : {}", config.device_id);

        let host_client = JsClient::new(JsClientBuilder {
            nats_remote_args: NatsRemoteArgs {
                nats_url: nats_url.clone(),
                ..Default::default()
            },
            name: HOST_AGENT_CLIENT_NAME.to_string(),
            inbox_prefix: format!("{HOST_AGENT_INBOX_PREFIX}.{device_id_lowercase}"),
            credentials: Default::default(),
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(29)),
            listeners: vec![with_event_listeners(get_event_listeners())],
        })
        .await?;

        Ok(Self {
            client: host_client,
        })
    }

    async fn stop(&self) -> HostAgentResult<()> {
        self.client.close().await?;
        Ok(())
    }
}
