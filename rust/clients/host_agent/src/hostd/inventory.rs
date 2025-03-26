/*
  This client is associated with the:
    - HPOS account
    - host user

  This client is responsible for publishing to the inventory suject(s):
    - `INVENTORY.<agent_pubkey>.update

  This client does not subject to or consume any inventory subjects.
*/

use std::{path::Path, time::Duration};

use futures::TryFutureExt;
use hpos_hal::inventory::HoloInventory;
use inventory::INVENTORY_UPDATE_SUBJECT;
use nats_utils::{
    jetstream_client::JsClient,
    types::{PublishInfo, ServiceError},
};
use tokio::time::sleep;

pub fn should_check_inventory(
    start: chrono::DateTime<chrono::Utc>,
    check_interval_duration: chrono::TimeDelta,
) -> bool {
    let now = chrono::Utc::now();
    now.signed_duration_since(start) > check_interval_duration
}

/// Periodically checks for inventory changes and publishes them on a NATS subject.
/// Never returns.
pub async fn run(
    host_client: JsClient,
    host_id: &str,
    inventory_file_path_str: &str,
    host_inventory_check_interval_sec: u64,
) -> ! {
    log::info!("Host Agent Client: starting Inventory job...");

    let interval = tokio::time::Duration::from_secs(host_inventory_check_interval_sec);
    let check_interval_duration = chrono::TimeDelta::seconds(interval.as_secs() as i64);
    let mut last_check_time = chrono::Utc::now() - interval;

    let pubkey_lowercase = host_id.to_lowercase();

    let inventory_file_path = Path::new(inventory_file_path_str);

    // used in case of an error when reading/parsing the persisted inventory
    let retry_in_default = std::time::Duration::from_secs(10);
    let mut retry_in = retry_in_default;
    let bump_retry_in =
        |ref mut retry_in| *retry_in = std::cmp::min(*retry_in * 2, Duration::from_secs(1800));

    loop {
        // Periodically check inventory and compare against latest state (in-memory)
        if should_check_inventory(last_check_time, check_interval_duration) {
            log::debug!("Checking Host inventory...");

            let current_inventory = HoloInventory::from_host();
            match inventory_file_path
                // check if the file exists
                .metadata()
                .map_err(hpos_hal::inventory::InventoryError::from)
                // on success try to parse it as an inventory
                .and_then(|_| HoloInventory::load_from_file(inventory_file_path_str))
                // log errors if it couldn't be read or parsed, and continue to publish the current one and persist it
                .inspect_err(|e| {
                    log::error!(
                        "Failed to read host inventory from file at {inventory_file_path_str}: {e}"
                    )
                })
                .map(|loaded_inventory| loaded_inventory == current_inventory)
            {
                Ok(true) => {
                    log::debug!("Host Inventory has not changed.");
                }

                Ok(false) | Err(_) => {
                    log::debug!(
                        "Host Inventory has changed or could not be loaded from {inventory_file_path_str}.  About to push update to Orchestrator"
                    );
                    let authenticated_user_inventory_subject =
                        format!("INVENTORY.{pubkey_lowercase}.{INVENTORY_UPDATE_SUBJECT}");

                    let payload = async {
                        serde_json::to_vec(&current_inventory).map_err(anyhow::Error::from)
                    }
                    .and_then(|payload_bytes| async {
                        Ok(PublishInfo {
                            subject: authenticated_user_inventory_subject.clone(),
                            msg_id: chrono::Utc::now().to_string(),
                            data: payload_bytes,
                            headers: None,
                        })
                    })
                    .and_then(|payload| host_client.publish(payload).map_err(anyhow::Error::from));

                    if let Err(e) = payload.await.map_err(|e| {
                        ServiceError::nats(
                            e.to_string(),
                            Some("Failed to publish host inventory.".to_string()),
                        )
                    }) {
                        log::error!("error publishing latest inventory on {authenticated_user_inventory_subject}: {e}\n. will retry in {retry_in:#?}");
                        sleep(retry_in).await;
                        bump_retry_in(retry_in);
                        continue;
                    };
                    if let Err(e) =
                        current_inventory
                            .save_to_file(inventory_file_path_str)
                            .map_err(|e| {
                                ServiceError::internal(
                                    e.to_string(),
                                    Some(format!(
                                    "Failed to save host inventory to file {inventory_file_path_str}"
                                )),
                                )
                            })
                    {
                        log::error!("{e}. will retry in {retry_in:#?}");
                        sleep(retry_in).await;
                        bump_retry_in(retry_in);
                        continue;
                    };
                }
            }

            retry_in = retry_in_default;
            last_check_time = chrono::Utc::now();
        }

        sleep(interval).await;
    }
}
