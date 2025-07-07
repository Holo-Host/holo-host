pub mod client;
pub mod config;
pub mod keys;
pub mod service;
pub(crate) mod utils;

use chrono::Duration as ChronoDuration;
use tokio::sync::broadcast;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

pub async fn run(
    device_id: &str,
    mut keys: keys::Keys,
    hub_url: &str,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> HostAgentResult<keys::Keys> {
    let mut auth_guard_client: Option<async_nats::Client> = None;

    // Set wait time to 1 sec to start auth immediately on first iteration
    let mut sleep_duration = std::time::Duration::from_secs(1);

    loop {
        tokio::select! {
            _ = tokio::time::sleep(sleep_duration) => {
                // Update wait time to 24 hours for all following iterations
                sleep_duration = ChronoDuration::hours(24).to_std()?;

                log::debug!("About to run the Hosting Agent Authentication Service");

                // Create auth client with shutdown listener
                let auth_result = tokio::select! {
                    result = service::authorize_host(device_id, keys.clone(), hub_url) => {
                        result
                    },
                    _ = shutdown_rx.recv() => {
                        log::info!("Auth shutdown signal received during authentication attempt for device '{}'", device_id);
                        // Drain any existing client before returning
                        if let Some(client) = auth_guard_client.take() {
                            if let Err(e) = client.drain().await {
                                log::warn!("Failed to drain auth client during shutdown: {}", e);
                            }
                        }
                        return Err(HostAgentError::service_failed(
                            "authentication service",
                            "shutdown during authentication attempt"
                        ));
                    }
                };

                // check auth result
                match auth_result {
                    Ok((new_keys, client)) => {
                        // Update the `auth_guard_client` to allow for proper clean-up
                        // if shutdown is called during this flow:
                        auth_guard_client = Some(client);
                        keys = new_keys;

                        // If authenticated creds exist, then auth call was successful
                        // and the updated credentials were saved to disk.
                        // We should close/drain out the auth client and exit loop.
                        if let keys::AuthCredType::Authenticated(_) = keys.creds {
                            utils::drain_client(auth_guard_client).await;
                            break;
                        }

                        // Otherwise, the auth call was unsuccessful.
                        auth_guard_client = utils::handle_unsuccessful_auth_call(device_id, auth_guard_client).await?;

                        // Close and drain auth client before waiting another wait interval..
                        auth_guard_client = utils::drain_client(auth_guard_client).await;
                    }
                    Err(e) => {
                        // If failed to create the auth client, continue in the loop & retry after 24 hours
                        log::error!("Authentication failed for device '{}': {}. Will retry in 24 hours.", device_id, e);
                    }
                }
            }
            _ = shutdown_rx.recv() => {
                log::info!("Auth shutdown signal received - auth service is shutting down.");
                if let Some(client) = auth_guard_client.take() {
                    if let Err(e) = client.drain().await {
                        log::warn!("Failed to drain auth client during shutdown: {}", e);
                    }
                }
                break;
            }
        }
    }

    Ok(keys)
}
