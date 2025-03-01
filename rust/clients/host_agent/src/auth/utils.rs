use crate::{auth, keys};
use anyhow::Result;
use data_encoding::BASE64URL_NOPAD;
use hpos_hal::inventory::HoloInventory;
use inventory::HOST_UNAUTHENTICATED_SUBJECT;

/// Encode a json string into a b64 string
pub fn json_to_base64(json_data: &str) -> Result<String, serde_json::Error> {
    let parsed_json: serde_json::Value = serde_json::from_str(json_data)?;
    let json_string = serde_json::to_string(&parsed_json)?;
    let encoded = BASE64URL_NOPAD.encode(json_string.as_bytes());
    Ok(encoded)
}

pub async fn run_auth_loop(mut keys: keys::Keys) -> Result<keys::Keys, async_nats::Error> {
    let mut start = chrono::Utc::now();
    let pubkey_lowercase = keys.host_pubkey.to_string().to_lowercase();

    loop {
        log::debug!("About to run the Hosting Agent Authentication Service");
        let auth_guard_client: async_nats::Client;
        (keys, auth_guard_client) = auth::init::run(keys).await?;

        // If authenicated creds exist, then auth call was successful.
        // Close buffer, exit loop, and return.
        if let keys::AuthCredType::Authenticated(_) = keys.creds {
            auth_guard_client.drain().await?;
            break;
        }

        // Otherwise, send diagonostics and wait 24hrs, then exit while loop and retry auth.
        // TODO: Discuss interval for sending diagnostic reports and wait duration before retrying auth with team.
        let now = chrono::Utc::now();
        let max_time_interval = chrono::TimeDelta::hours(24);

        while max_time_interval > now.signed_duration_since(start) {
            let unauthenticated_user_inventory_subject =
                format!("INVENTORY.{HOST_UNAUTHENTICATED_SUBJECT}.{pubkey_lowercase}.update");
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
            tokio::time::sleep(chrono::TimeDelta::hours(24).to_std()?).await;
        }

        // Close and drain internal buffer before exiting to make sure all messages are sent.
        auth_guard_client.drain().await?;
        start = chrono::Utc::now();
    }

    Ok(keys)
}
