/*
  This client is associated with the:
    - HPOS account
    - host user

  This client is responsible for publishing to the inventory subject(s):
    - `INVENTORY.<agent_device_id>.update

  This client does not subscribe to or consume any inventory subjects.
*/

use std::{path::Path, time::Duration};

use futures::TryFutureExt;
use hpos_hal::inventory::HoloInventory;
use inventory::INVENTORY_UPDATE_SUBJECT;
use nats_utils::{jetstream_client::JsClient, types::PublishInfo};
use tokio::sync::broadcast;
use tokio::time::sleep;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

pub fn should_check_inventory(
    start: chrono::DateTime<chrono::Utc>,
    check_interval_duration: chrono::TimeDelta,
) -> bool {
    let now = chrono::Utc::now();
    now.signed_duration_since(start) > check_interval_duration
}

/// Periodically checks for inventory changes and publishes them on a NATS subject.
pub async fn run(
    host_client: &mut JsClient,
    device_id: &str,
    inventory_file_path_str: &str,
    host_inventory_check_interval_sec: u64,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> HostAgentResult<()> {
    log::info!("Host Agent Client: starting Inventory job...");

    // Validate input parameters
    if device_id.is_empty() {
        return Err(HostAgentError::service_failed(
            "inventory service validation",
            "Device ID cannot be empty",
        ));
    }
    if inventory_file_path_str.is_empty() {
        return Err(HostAgentError::service_failed(
            "inventory service validation",
            "Inventory file path cannot be empty",
        ));
    }

    let interval = tokio::time::Duration::from_secs(host_inventory_check_interval_sec);
    let check_interval_duration = chrono::TimeDelta::seconds(interval.as_secs() as i64);
    let mut last_check_time = chrono::Utc::now() - interval;

    let device_id_lowercase = device_id.to_lowercase();

    let inventory_file_path = Path::new(inventory_file_path_str);

    // used in case of an error when reading/parsing the persisted inventory
    let retry_in_default = std::time::Duration::from_secs(10);
    let mut retry_in = retry_in_default;
    let bump_retry_in = |retry_in: &mut Duration| {
        *retry_in = std::cmp::min(*retry_in * 2, Duration::from_secs(1800))
    };

    // Create additional shutdown receivers for inner select statements
    let mut retry_shutdown_rx = shutdown_rx.resubscribe();
    let mut save_shutdown_rx = shutdown_rx.resubscribe();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Shutdown signal received in inventory service");
                break;
            }
            _ = async {
        // Periodically check inventory and compare against latest state (in-memory)
        if should_check_inventory(last_check_time, check_interval_duration) {
            log::debug!("Checking Host inventory...");

            let current_inventory = HoloInventory::from_host();
                    let inventory_changed = match inventory_file_path
                // check if the file exists
                .metadata()
                        .map_err(|e| HostAgentError::service_failed(
                            "inventory file metadata check",
                            &format!("Failed to get metadata for inventory file: {}", e),
                        ))
                // on success try to parse it as an inventory
                        .and_then(|_| HoloInventory::load_from_file(inventory_file_path_str)
                            .map_err(|e| HostAgentError::service_failed(
                                "inventory file loading",
                                &format!("Failed to load inventory from file: {}", e),
                            )))
                .map(|loaded_inventory| loaded_inventory == current_inventory)
            {
                Ok(true) => {
                    log::debug!("Host Inventory has not changed.");
                            false
                }

                Ok(false) | Err(_) => {
                    log::debug!(
                        "Host Inventory has changed or could not be loaded from {inventory_file_path_str}.  About to push update to Orchestrator"
                    );
                            true
                        }
                    };

                    if inventory_changed {
                    let authenticated_user_inventory_subject =
                        format!("INVENTORY.{device_id_lowercase}.{INVENTORY_UPDATE_SUBJECT}");

                        // Try to publish inventory
                        let publish_result = async {
                            serde_json::to_vec(&current_inventory).map_err(|e| HostAgentError::service_failed(
                                "inventory serialization",
                                &format!("Failed to serialize inventory: {}", e),
                            ))
                    }
                    .and_then(|payload_bytes| async {
                        Ok(PublishInfo {
                            subject: authenticated_user_inventory_subject.clone(),
                            msg_id: chrono::Utc::now().to_string(),
                            data: payload_bytes,
                            headers: None,
                        })
                    })
                        .and_then(|payload| {
                            host_client.publish(payload).map_err(|e| HostAgentError::service_failed(
                                "inventory publishing",
                                &format!("Failed to publish host inventory: {}", e),
                            ))
                        })
                        .await;

                        if let Err(e) = publish_result {
                        log::error!("error publishing latest inventory on {authenticated_user_inventory_subject}: {e}\n. will retry in {retry_in:#?}");
                            // Sleep for retry delay with shutdown handling
                            tokio::select! {
                                _ = sleep(retry_in) => {
                                    bump_retry_in(&mut retry_in);
                                }
                                _ = retry_shutdown_rx.recv() => {
                                    log::info!("Shutdown signal received during retry delay");
                                    return Err(HostAgentError::service_failed("inventory service", "Shutdown during retry"));
                                }
                            }
                            // Don't update last_check_time, so it will retry immediately
                            return Ok(());
                        }

                        // Try to save inventory to file
                        if let Err(e) = current_inventory
                            .save_to_file(inventory_file_path_str)
                            .map_err(|e| HostAgentError::service_failed(
                                "inventory file saving",
                                &format!("Failed to save host inventory to file {}: {}", inventory_file_path_str, e),
                            ))
                        {
                            log::error!("{e}. will retry in {retry_in:#?}");
                            // Sleep for retry delay with shutdown handling
                            tokio::select! {
                                _ = sleep(retry_in) => {
                                    bump_retry_in(&mut retry_in);
                                }
                                _ = save_shutdown_rx.recv() => {
                                    log::info!("Shutdown signal received during retry delay");
                                    return Err(HostAgentError::service_failed("inventory service", "Shutdown during retry"));
                                }
                            }
                            // Don't update last_check_time, so it will retry immediately
                            return Ok(());
                        }

                        // Success - reset retry timer and update last check time
            retry_in = retry_in_default;
            last_check_time = chrono::Utc::now();
                    }
        }

        sleep(interval).await;
                Ok(())
            } => {
                // If here, the inventory check completed. Continue loop..
            }
        }
    }

    log::info!("Inventory service shutting down gracefully");
    Ok(())
}
