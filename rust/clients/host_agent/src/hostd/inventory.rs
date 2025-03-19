/*
  This client is associated with the:
    - HPOS account
    - host user

  This client is responsible for publishing to the inventory suject(s):
    - `INVENTORY.<agent_pubkey>.update

  This client does not subject to or consume any inventory subjects.
*/

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

pub async fn run(
    host_client: JsClient,
    host_id: &str,
    _inventory_file_path: &str,
    host_inventory_check_interval_sec: u64,
    starting_inventory: HoloInventory,
) -> anyhow::Result<()> {
    log::info!("Host Agent Client: starting Inventory job...");
    let inventory_file_path = "host_temp_dir";
    log::info!("Host Agent Client: inventory_file_path={inventory_file_path}");

    // Store latest inventory record in memory
    starting_inventory
        .save_to_file(inventory_file_path)
        .map_err(|e| {
            ServiceError::internal(
                e.to_string(),
                Some("Failed to save host inventory to file.".to_string()),
            )
        })?;

    log::info!("Host Agent Client: saved inventory to file...");

    let interval = tokio::time::Duration::from_secs(host_inventory_check_interval_sec);
    let check_interval_duration = chrono::TimeDelta::seconds(interval.as_secs() as i64);
    let mut last_check_time = chrono::Utc::now();

    let pubkey_lowercase = host_id.to_lowercase();

    let mut first_start = true;
    loop {
        // Periodically check inventory and compare against latest state (in-memory)
        if first_start || should_check_inventory(last_check_time, check_interval_duration) {
            log::debug!("Checking Host inventory...");

            let current_inventory = HoloInventory::from_host();
            if first_start
                || HoloInventory::load_from_file(inventory_file_path).map_err(|e| {
                    ServiceError::internal(
                        e.to_string(),
                        Some("Failed to read host inventory from file.".to_string()),
                    )
                })? != current_inventory
            {
                first_start = false;

                log::debug!("Host Inventory has changed.  About to push update to Orchestrator");
                let authenticated_user_inventory_subject =
                    format!("INVENTORY.{pubkey_lowercase}.{INVENTORY_UPDATE_SUBJECT}");

                let payload_bytes = serde_json::to_vec(&current_inventory)?;

                let payload = PublishInfo {
                    subject: authenticated_user_inventory_subject,
                    msg_id: chrono::Utc::now().to_string(),
                    data: payload_bytes,
                    headers: None,
                };

                host_client.publish(payload).await.map_err(|e| {
                    ServiceError::nats(
                        e.to_string(),
                        Some("Failed to publish host inventory.".to_string()),
                    )
                })?;
                current_inventory
                    .save_to_file(inventory_file_path)
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to save host inventory to file.".to_string()),
                        )
                    })?;
            } else {
                log::debug!("Host Inventory has not changed.");
            }

            last_check_time = chrono::Utc::now();
        }

        sleep(interval).await;
    }
}
