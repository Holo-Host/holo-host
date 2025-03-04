/*
 This client is associated with the:
    - WORKLOAD account
    - host user

This client is responsible for subscribing to workload streams that handle:
    - installing new workloads onto the hosting device
    - removing workloads from the hosting device
    - sending workload status upon request
    - sending out active periodic workload reports
*/

use anyhow::Result;
use hpos_hal::inventory::HoloInventory;
use inventory::INVENTORY_UPDATE_SUBJECT;
use nats_utils::{jetstream_client::JsClient, types::PublishInfo};
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
    host_pubkey: &str,
    inventory_file_path: &str,
    host_inventory_check_interval_sec: u64,
) -> Result<(), async_nats::Error> {
    log::info!("Host Agent Client: starting Inventory job...");

    // Store latest inventory record in memory
    let starting_inventory = HoloInventory::from_host();
    starting_inventory.save_to_file(inventory_file_path)?;

    let one_hour_interval = tokio::time::Duration::from_secs(host_inventory_check_interval_sec);
    let check_interval_duration = chrono::TimeDelta::seconds(one_hour_interval.as_secs() as i64);
    let mut last_check_time = chrono::Utc::now();

    let pubkey_lowercase = host_pubkey.to_string().to_lowercase();

    loop {
        // Periodically check inventory and compare against latest state (in-memory)
        if should_check_inventory(last_check_time, check_interval_duration) {
            log::debug!("Checking Host inventory...");

            let current_inventory = HoloInventory::from_host();
            if HoloInventory::load_from_file(inventory_file_path)? != current_inventory {
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

                host_client.publish(payload).await?;
                current_inventory.save_to_file(inventory_file_path)?;
            } else {
                log::debug!("Host Inventory has not changed.");
            }

            last_check_time = chrono::Utc::now();
        }

        sleep(one_hour_interval).await;
    }
}
