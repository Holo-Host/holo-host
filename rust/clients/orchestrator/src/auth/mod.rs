mod service;
mod utils;

use mongodb::Client as MongoDBClient;
use tokio::sync::broadcast;

use crate::errors::OrchestratorError;

pub async fn run(
    auth_client: async_nats::Client,
    db_client: MongoDBClient,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), OrchestratorError> {
    log::info!("Starting auth service...");
    let _ = service::run(auth_client.clone(), db_client)
        .await
        .map_err(|e| OrchestratorError::Client(format!("Inventory client error: {:?}", e)));

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Auth client service shutting down");
                break;
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                // NB: Keep the service alive until shutdown signal is received
                // Add any auth service specific logic here
            }
        }
    }

    // Close auth client
    log::info!("Auth client stopped");
    auth_client
        .drain()
        .await
        .map_err(|e| OrchestratorError::Shutdown(format!("Failed to drain auth client: {}", e)))?;

    Ok(())
}
