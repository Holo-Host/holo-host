use crate::nats_clients::{auth::AuthClient, admin::AdminClient, OrchestratorClient};
use crate::{config::OrchestratorConfig, errors::OrchestratorError, Args};
use crate::nats_services;

use anyhow::{Context, Result};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use tokio::sync::broadcast;
use tokio::task::JoinSet;

pub struct Orchestrator {
    _config: OrchestratorConfig,
    services: JoinSet<Result<(), OrchestratorError>>,
    shutdown_tx: broadcast::Sender<()>,
}

impl Orchestrator {
    pub async fn initialize(args: Args) -> Result<Self, OrchestratorError> {
        let config = OrchestratorConfig::from_args(args)?;
        let (shutdown_tx, _) = broadcast::channel(1);
        let mut services = JoinSet::new();
        
        // Setup database
        let db_client = setup_database(&config).await?;
        
        // Setup auth client service (auth service)
        let auth_client = AuthClient::start(&config).await?;
        
        // Setup admin client services (workload and inventory)
        let admin_client = AdminClient::start(&config).await?;
        
        // Spawn services with proper error handling
        services.spawn(nats_services::run_auth(auth_client.client, db_client.clone(), shutdown_tx.subscribe()));
        services.spawn(nats_services::run_workload_and_inventory(admin_client.client, db_client.clone(), shutdown_tx.subscribe()));
        
        Ok(Self { 
            services, 
            shutdown_tx, 
            _config: config,
        })
    }
    
    pub async fn run(mut self) -> Result<(), OrchestratorError> {
        log::info!("Starting orchestrator with {} services", self.services.len());
        
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
        
        log::info!("Successfully shut down orchestrator");
        Ok(())
    }
}

async fn setup_database(config: &OrchestratorConfig) -> Result<MongoDBClient, OrchestratorError> {
    log::info!("Connecting to mongodb at {}", config.mongo_uri);
    let db_client_options = ClientOptions::parse(&config.mongo_uri)
        .await
        .context(format!("mongo db client: connecting to {}", config.mongo_uri))
        .map_err(|e| OrchestratorError::Configuration(e.to_string()))?;
    
    MongoDBClient::with_options(db_client_options)
        .map_err(OrchestratorError::Database)
}