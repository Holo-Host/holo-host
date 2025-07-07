use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;

use crate::types::nats_clients::OrchestratorClient;
use crate::{errors::OrchestratorError, types::config::OrchestratorConfig};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Manager";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX.orchestrator";

pub struct AuthClient {
    pub client: async_nats::Client,
    _creds_path: PathBuf,
}

impl OrchestratorClient for AuthClient {
    type Output = Self;

    async fn start(config: &OrchestratorConfig) -> Result<Self, OrchestratorError> {
        log::info!("Starting orchestrator auth service...");
        log::info!("auth_creds_path : {:?}", config.auth_creds_path);
        let creds_path = config.auth_creds_path.clone();
        let nats_url = &config.nats_remote_args.nats_url.as_ref();
        log::info!("auth nats_url : {nats_url:?}");

        let nats_connect_timeout_secs: u64 = 180;
        let auth_client = tokio::select! {
            client = async {loop {
                let auth_client = async_nats::ConnectOptions::new()
                    .name(ORCHESTRATOR_AUTH_CLIENT_NAME.to_string())
                    .custom_inbox_prefix(ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX.to_string())
                    .ping_interval(Duration::from_secs(10))
                    .request_timeout(Some(Duration::from_secs(30)))
                    .credentials_file(&creds_path.clone()).await.map_err(|e| anyhow::anyhow!("Error loading credentials file: {e}"))?
                    .connect(nats_url)
                    .await
                    .map_err(|e| anyhow::anyhow!("Connecting Orchestrator Auth Client to NATS via {nats_url:?}: {e}"));

                match auth_client {
                    Ok(client) => break Ok::<async_nats::Client, async_nats::Error>(client),
                    Err(e) => {
                        let duration = tokio::time::Duration::from_millis(100);
                        log::warn!("{}, retrying in {duration:?}", e);
                        tokio::time::sleep(duration).await;
                    }
                }
            }} => client?,
            _ = {
                log::debug!("Will time out waiting for NATS after {nats_connect_timeout_secs:?}...");
                tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
                } => {
                return Err(OrchestratorError::Nats(anyhow::anyhow!("Timed out waiting for NATS on {nats_url:?}").into()));
            }
        };

        log::debug!("Orchestrator auth client is ready");
        Ok(Self {
            client: auth_client,
            _creds_path: creds_path,
        })
    }

    async fn stop(&self) -> Result<(), OrchestratorError> {
        self.client
            .drain()
            .await
            .map_err(|e| OrchestratorError::Shutdown(format!("Failed to drain auth client: {}", e)))
    }
}
