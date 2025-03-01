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
use inventory::HOST_AUTHENTICATED_SUBJECT;
use tokio::time::sleep;
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

    // Store latest inventory record in memory
    let mut in_memory_cache = HoloInventory::from_host();

    let one_hour_interval = tokio::time::Duration::from_secs(3600); // 1 hour in seconds
    let check_interval_duration = chrono::TimeDelta::seconds(one_hour_interval.as_secs() as i64);
    let mut last_check_time = chrono::Utc::now();

    let pubkey_lowercase = host_pubkey.to_lowercase();

    loop {
        // Periodically check inventory and compare against latest state (in-memory)
        if should_check_inventory(last_check_time, check_interval_duration) {
            log::debug!("Host Inventory has changed.  About to push update to Orchestrator");
            let current_inventory = HoloInventory::from_host();
            if in_memory_cache != current_inventory {
                let authenticated_user_inventory_subject =
                    format!("INVENTORY.{HOST_AUTHENTICATED_SUBJECT}.{pubkey_lowercase}.update");

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
            last_check_time = chrono::Utc::now();
        } else {
            log::debug!("Host Inventory has not changed.");
        }

        sleep(one_hour_interval).await;
    }
}
