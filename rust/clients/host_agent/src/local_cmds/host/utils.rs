use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::task::JoinSet;

const DEFAULT_SHUTDOWN_TIMEOUT_SECS: u64 = 30;
const SHUTDOWN_GRACE_PERIOD_MS: u64 = 100;

/// Performs a graceful shutdown of the host agent services and leaf server
pub async fn graceful_shutdown(
    shutdown_reason: &str,
    shutdown_tx: broadcast::Sender<()>,
    mut services: JoinSet<HostAgentResult<()>>,
    mut leaf_server: nats_utils::leaf_server::LeafServer,
) -> HostAgentResult<()> {
    // Initiate graceful shutdown process
    // Send shutdown signal to all services
    log::info!("Initiating graceful shutdown due to: {}", shutdown_reason);

    if let Err(e) = shutdown_tx.send(()) {
        log::warn!("Failed to send shutdown signal to services. err={}", e);
    }

    // Give services a moment to process the shutdown signal
    tokio::time::sleep(Duration::from_millis(SHUTDOWN_GRACE_PERIOD_MS)).await;
    services.shutdown().await;

    // Wait for all services to complete with timeout
    let shutdown_timeout = Duration::from_secs(DEFAULT_SHUTDOWN_TIMEOUT_SECS);
    let shutdown_result = tokio::time::timeout(shutdown_timeout, async {
        let mut completed_services = 0;
        while let Some(result) = services.join_next().await {
            completed_services += 1;
            match result {
                Ok(Ok(())) => log::debug!("Service {} exited successfully", completed_services),
                Ok(Err(e)) => log::warn!("Service {} exited with error: {}", completed_services, e),
                Err(e) => log::error!("Task join error for service {}: {}", completed_services, e),
            }
        }
        completed_services
    })
    .await;

    match shutdown_result {
        Ok(service_count) => log::info!("All {} services shut down successfully", service_count),
        Err(_) => {
            log::warn!(
                "Shutdown timeout reached after {} seconds - forcing abort of remaining services",
                DEFAULT_SHUTDOWN_TIMEOUT_SECS
            );
            services.abort_all();
        }
    }

    // Always close the leaf server regardless of shutdown reason
    log::info!("Closing leaf server...");
    if let Err(e) = leaf_server.close().await {
        log::error!(
            "Failed to close leaf server cleanly: {}. \
             This may leave the server port in use temporarily.",
            e
        );
        // Return error for leaf server close failure only if it was due to service failure
        if shutdown_reason == "service_failure" {
            return Err(HostAgentError::service_failed(
                "leaf server shutdown",
                &format!(
                    "Service failure occurred and leaf server failed to close cleanly: {}",
                    e
                ),
            ));
        }
    } else {
        log::info!("Leaf server closed successfully");
    }

    log::info!("Host agent shutdown completed successfully");
    Ok(())
}
