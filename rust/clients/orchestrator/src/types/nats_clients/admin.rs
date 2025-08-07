use anyhow::Result;
use nats_utils::{
    jetstream_client::{get_event_listeners, with_event_listeners, JsClient},
    types::JsClientBuilder,
};
use std::time::Duration;
use std::vec;

use crate::types::nats_clients::OrchestratorClient;
use crate::{errors::OrchestratorError, types::config::OrchestratorConfig};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "_ADMIN_INBOX.orchestrator";

pub struct AdminClient {
    pub client: nats_utils::jetstream_client::JsClient,
}

impl OrchestratorClient for AdminClient {
    type Output = Self;

    async fn start(config: &OrchestratorConfig) -> Result<Self, OrchestratorError> {
        log::info!("Starting orchestrator admin client...");

        let nats_url = config.nats_remote_args.nats_url.clone();
        log::info!("admin nats_url : {nats_url:?}");

        let admin_client = JsClient::new(JsClientBuilder {
            nats_remote_args: config.nats_remote_args.clone(),
            name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
            inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
            credentials: Default::default(),
            request_timeout: Some(Duration::from_secs(29)),
            ping_interval: Some(Duration::from_secs(10)),
            listeners: vec![with_event_listeners(get_event_listeners())],
        })
        .await
        .map_err(|e| OrchestratorError::Client(format!("Failed to start admin client: {}", e)))?;

        log::debug!("Orchestrator admin client is ready");
        Ok(Self {
            client: admin_client,
        })
    }

    async fn stop(&self) -> Result<(), OrchestratorError> {
        self.client.close().await.map_err(|e| {
            OrchestratorError::Shutdown(format!("Failed to close admin client: {}", e))
        })
    }
}
