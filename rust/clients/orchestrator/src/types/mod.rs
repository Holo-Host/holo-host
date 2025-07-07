pub mod config;
pub mod nats_clients;

use crate::types::nats_clients::{admin::AdminClient, auth::AuthClient, OrchestratorClient};
use crate::{admin, auth};
use crate::{errors::OrchestratorError, types::config::OrchestratorConfig, Args};

use anyhow::Result;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

pub struct Orchestrator {
    _config: OrchestratorConfig,
    services: JoinSet<Result<(), OrchestratorError>>,
    shutdown_tx: broadcast::Sender<()>,
    auth_client: AuthClient,
    admin_client: AdminClient,
}

impl Orchestrator {
    pub async fn initialize(args: Args) -> Result<Self, OrchestratorError> {
        let config = OrchestratorConfig::from_args(args)?;

        // Setup database
        let db_client = config.setup_database().await?;

        // Setup service shutdown mechanism
        let (shutdown_tx, _) = broadcast::channel(1);
        let mut services = JoinSet::new();

        // Setup auth client and its service
        let auth_client = AuthClient::start(&config).await?;
        services.spawn(auth::run(
            auth_client.client.clone(),
            db_client.clone(),
            shutdown_tx.subscribe(),
        ));

        // Setup admin client and its services
        let admin_client = AdminClient::start(&config).await?;
        services.spawn(admin::run(
            admin_client.client.clone(),
            db_client.clone(),
            shutdown_tx.subscribe(),
        ));

        Ok(Self {
            services,
            shutdown_tx,
            _config: config,
            auth_client,
            admin_client,
        })
    }

    pub async fn run(mut self) -> Result<(), OrchestratorError> {
        log::info!(
            "Starting orchestrator with {} services",
            self.services.len()
        );

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Shutdown signal received");
            }
            result = self.services.join_next() => {
                if let Some(Err(e)) = result {
                    log::error!("Service failed: {}", e);
                    return Err(OrchestratorError::Client(format!("Service failure: {}", e)));
                }
            }
        }

        // Shutdown gracefully
        log::info!("Initiating graceful shutdown...");
        let _ = self.shutdown_tx.send(());
        self.services.shutdown().await;

        // Wait for all services to complete
        while let Some(result) = self.services.join_next().await {
            match result {
                Ok(Ok(())) => log::debug!("Service exited successfully"),
                Ok(Err(e)) => log::warn!("Service exited with error: {}", e),
                Err(e) => log::error!("Task join error: {}", e),
            }
        }

        // Stop the clients gracefully
        log::info!("Stopping clients...");
        if let Err(e) = self.auth_client.stop().await {
            log::warn!("Failed to stop auth client: {}", e);
        }
        if let Err(e) = self.admin_client.stop().await {
            log::warn!("Failed to stop admin client: {}", e);
        }

        log::info!("Successfully shut down orchestrator");
        Ok(())
    }
}
