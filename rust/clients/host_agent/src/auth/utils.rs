use data_encoding::BASE64URL_NOPAD;
use hpos_hal::inventory::HoloInventory;

use crate::local_cmds::host::errors::HostAgentResult;

// Validates and normalizes JSON, then encodes it as base64.
pub fn json_to_base64(json_data: &str) -> HostAgentResult<String> {
    let parsed_json: serde_json::Value = serde_json::from_str(json_data)?;
    let normalized_json = serde_json::to_string(&parsed_json)?;

    let encoded = BASE64URL_NOPAD.encode(normalized_json.as_bytes());
    Ok(encoded)
}

pub async fn drain_client(
    mut auth_guard_client: Option<async_nats::Client>,
) -> Option<async_nats::Client> {
    if let Some(client) = auth_guard_client.take() {
        if let Err(e) = client.drain().await {
            log::warn!("Failed to drain auth client after failed auth: {}", e);
            return Some(client);
        }
    }

    None
}

pub async fn handle_unsuccessful_auth_call(
    device_id: &str,
    auth_guard_client: Option<async_nats::Client>,
) -> HostAgentResult<Option<async_nats::Client>> {
    // If auth was unsuccessful, we should take 3 actions :
    // 1. send inventory of the machine that failed
    // 2. drain the auth client buffer to clear out any idle messages
    // 3. stay in loop for auth to automatically retry at next 24hr interval
    let device_id_lowercase = device_id.to_lowercase();
    let unauthenticated_user_inventory_subject =
        format!("INVENTORY.unauthenticated.{}.update", device_id_lowercase);

    let inventory = HoloInventory::from_host();
    let payload_bytes = serde_json::to_vec(&inventory)?;

    if let Some(client) = &auth_guard_client {
        if let Err(e) = client
            .publish(
                unauthenticated_user_inventory_subject.clone(),
                payload_bytes.into(),
            )
            .await
        {
            log::error!(
                "Failed to publish inventory for unauthenticated device '{}' to subject '{}': {}",
                device_id,
                unauthenticated_user_inventory_subject,
                e
            );
        };
    }

    Ok(auth_guard_client)
}
