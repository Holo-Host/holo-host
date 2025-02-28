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
use util_libs::nats::{jetstream_client::JsClient, types::PublishInfo};

pub fn should_check_inventory(
    start: chrono::DateTime<chrono::Utc>,
    check_interval_duration: chrono::TimeDelta,
) -> bool {
    let now = chrono::Utc::now();
    now.signed_duration_since(start) > check_interval_duration
}

pub async fn run(host_client: JsClient, host_pubkey: &str) -> Result<(), async_nats::Error> {
    log::info!("Host Agent Client: starting Inventory job...");
    log::info!("host_pubkey : {}", host_pubkey);

    let pubkey_lowercase = host_pubkey.to_string().to_lowercase();

    // ==================== Handle Inventory Check-Ups and Updates ====================
    // Store latest inventory record in memory
    let mut in_memory_cache = HoloInventory::from_host();

    // Periodically check inventory and compare against latest state (in-memory)
    let start = chrono::Utc::now();
    let check_interval_duration = chrono::TimeDelta::hours(1);

    loop {
        if should_check_inventory(start, check_interval_duration) {
            log::debug!("Host Inventory has changed.  About to push update to Orchestrator");
            let current_inventory = HoloInventory::from_host();
            if in_memory_cache != current_inventory {
                let authenticated_user_inventory_subject =
                    format!("INVENTORY.{}.update.authenticated", pubkey_lowercase);

                let payload_bytes = serde_json::to_vec(&current_inventory)?;

                let payload = PublishInfo {
                    subject: authenticated_user_inventory_subject,
                    msg_id: chrono::Utc::now().to_string(),
                    data: payload_bytes,
                    headers: None,
                };

                host_client.publish(payload).await?;

                in_memory_cache = current_inventory
            }
        } else {
            log::debug!("Host Inventory has not changed.");
        }
    }
}
