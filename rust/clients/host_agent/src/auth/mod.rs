pub mod config;
pub mod init;
pub mod utils;
use crate::{auth, keys};
use hpos_hal::inventory::HoloInventory;

pub async fn run_validation_loop(
    device_id: String,
    mut keys: keys::Keys,
    hub_url: &str,
) -> Result<keys::Keys, async_nats::Error> {
    let mut start = chrono::Utc::now();
    loop {
        log::debug!("About to run the Hosting Agent Authentication Service");
        let auth_guard_client: async_nats::Client;
        (keys, auth_guard_client) = auth::init::run(device_id.clone(), keys, hub_url).await?;

        // If authenicated creds exist, then auth call was successful.
        // Close buffer, exit loop, and return.
        if let keys::AuthCredType::Authenticated(_) = keys.creds {
            auth_guard_client.drain().await?;
            break;
        }

        // Otherwise, the auth call was unsuccessful and we should send inventory of the machine that failed
        // then wait 24hrs and retry auth..
        let now = chrono::Utc::now();
        let max_time_interval = chrono::TimeDelta::hours(24);
        if max_time_interval > now.signed_duration_since(start) {
            let device_id_lowercase = device_id.to_string().to_lowercase();
            let unauthenticated_user_inventory_subject =
                format!("INVENTORY.unauthenticated.{}.update", device_id_lowercase);
            let inventory = HoloInventory::from_host();
            let payload_bytes = serde_json::to_vec(&inventory)?;

            if let Err(e) = auth_guard_client
                .publish(unauthenticated_user_inventory_subject, payload_bytes.into())
                .await
            {
                log::error!(
                    "Encountered error when sending inventory as unauthenticated user. Err={:#?}",
                    e
                );
            };
        }

        // Close and drain internal buffer before exiting to make sure all messages are sent.
        auth_guard_client.drain().await?;

        tokio::time::sleep(chrono::TimeDelta::hours(24).to_std()?).await;
        start = chrono::Utc::now();
    }

    Ok(keys)
}
