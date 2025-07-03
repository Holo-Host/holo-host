mod utils;
pub mod inventory;
pub mod workloads;

use anyhow::Result;
use mongodb::{Client as MongoDBClient};
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use std::error::Error;

use crate::errors::OrchestratorError;

pub async fn run(
    admin_client: nats_utils::jetstream_client::JsClient,
    db_client: MongoDBClient,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), OrchestratorError> {    
    let mut admin_client_tasks = JoinSet::new();
    
    // Spawn workload service
    admin_client_tasks.spawn({
        let admin_client_clone = admin_client.clone();
        let db_client_clone = db_client.clone();
        async move {
            log::info!("Starting workload service...");
            workloads::run(admin_client_clone, db_client_clone).await
                .map_err(|e| OrchestratorError::Client(format!("Workload client error: {:?}", e)))
        }
    });
    
    // Spawn inventory service
    admin_client_tasks.spawn({
        let admin_client_clone = admin_client.clone();
        async move {
            log::info!("Starting inventory service...");
            inventory::run(admin_client_clone, db_client).await
                .map_err(|e: Box<dyn Error + Send + Sync + 'static>| OrchestratorError::Client(format!("Inventory client error: {:?}", e)))
        }
    });
    
    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Admin client services shutting down");
                break;
            }
            result = admin_client_tasks.join_next() => {
                if let Some(Err(e)) = result {
                    return Err(OrchestratorError::Client(format!("Timed out waiting for NATS on {e:?}")));
                }
            }
        }
    }

    // Shutdown gracefully 
    admin_client_tasks.shutdown().await;
    while let Some(result) = admin_client_tasks.join_next().await {
        match result {
            Ok(Ok(_)) => log::debug!("Client exited successfully"),
            Ok(Err(e)) => log::warn!("Admin client service (eg: workload or inventory) exited with error: {}", e),
            Err(e) => log::error!("Client task join error: {}", e),
        }
    }
    
    log::info!("Admin client stopped");
    admin_client.close().await
        .map_err(|e| OrchestratorError::Shutdown(format!("Failed to drain auth client: {}", e)))?;
            
    Ok(())
}
