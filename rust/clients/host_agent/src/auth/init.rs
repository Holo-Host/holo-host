/*
This client is associated with the:
    - AUTH account
    - auth guard user

Nb: Once the host and hoster are validated, and the host creds file is created,
...this client should safely close and then the `hostd.workload` manager should spin up.

This client is responsible for:
    - generating new key for host and accessing hoster key from provided config file
    - calling the host auth service to:
        - validate hoster hc pubkey and email
        - send the host pubkey to the orchestrator to register with the orchestrator key resovler
        - get user jwt from orchestrator and create user creds file with provided file path
    - returning the host pubkey and closing client cleanly
*/

use super::utils::json_to_base64;
use crate::{
    auth::config::HosterConfig,
    keys::{AuthCredType, Keys},
};
use anyhow::Result;
use async_nats::{HeaderMap, HeaderName, HeaderValue, RequestErrorKind};
use authentication::{
    types::{AuthGuardPayload, AuthJWTPayload, AuthJWTResult, AuthState},
    AUTH_SRV_SUBJ, VALIDATE_AUTH_SUBJECT,
};
use hpos_hal::inventory::HoloInventory;
// use nats_utils::{jetstream_client};
use std::str::FromStr;
use std::time::Duration;
use textnonce::TextNonce;

pub const HOST_AUTH_CLIENT_NAME: &str = "Host Auth";
pub const HOST_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX";

pub async fn run(
    device_id: String,
    mut host_agent_keys: Keys,
    hub_url: &str,
) -> Result<(Keys, async_nats::Client), async_nats::Error> {
    log::info!("Host Auth Client: Connecting to server...");
    log::trace!(
        "Host Agent Keys before authentication request: {:#?}",
        host_agent_keys
    );

    // ==================== Fetch Config File & Call NATS AuthCallout Service to Authenticate Host & Hoster =============================================
    let nonce = TextNonce::new().to_string();

    // Fetch Hoster Pubkey and email (from config)
    let mut auth_guard_payload = AuthGuardPayload::default();
    auth_guard_payload.device_id = device_id.clone();

    match HosterConfig::new().await {
        Ok(config) => {
            auth_guard_payload.host_pubkey = host_agent_keys.host_pubkey.to_string();
            auth_guard_payload.hoster_hc_pubkey = Some(config.hc_pubkey);
            auth_guard_payload.email = Some(config.email);
            auth_guard_payload.nonce = nonce;
        }
        Err(e) => {
            log::error!("Failed to locate Hoster config. Err={e}");
            auth_guard_payload.host_pubkey = host_agent_keys.host_pubkey.to_string();
            auth_guard_payload.nonce = nonce;
        }
    };
    auth_guard_payload = auth_guard_payload.try_add_signature(|p| host_agent_keys.host_sign(p))?;

    let user_auth_json = serde_json::to_string(&auth_guard_payload)?;
    let user_auth_token = json_to_base64(&user_auth_json)?;
    let user_creds_path = if let AuthCredType::Guard(creds) = host_agent_keys.creds.clone() {
        creds
    } else {
        return Err(async_nats::Error::from(
            "Failed to locate Auth Guard credentials",
        ));
    };
    let user_unique_inbox = &format!(
        "{}.{}",
        HOST_AUTH_CLIENT_INBOX_PREFIX,
        host_agent_keys.host_pubkey.to_lowercase()
    );

    // Connect to Nats server as auth guard and call NATS AuthCallout
    let nats_url = hub_url;
    let auth_guard_client = async_nats::ConnectOptions::new()
        .name(HOST_AUTH_CLIENT_NAME.to_string())
        .custom_inbox_prefix(user_unique_inbox.to_string())
        .ping_interval(Duration::from_secs(10))
        .request_timeout(Some(Duration::from_secs(30)))
        .token(user_auth_token)
        .credentials_file(&user_creds_path)
        .await?
        .connect(nats_url)
        .await?;

    let server_info = auth_guard_client.server_info();
    println!(
        "User connected to server on port {}.  Connection State: {:#?}",
        server_info.port,
        auth_guard_client.connection_state()
    );

    let server_node_id = server_info.server_id;
    log::trace!("Host Auth Client: Retrieved Node ID: {}", server_node_id);

    // ==================== Handle Host User and SYS Authoriation ============================================================
    let payload = AuthJWTPayload {
        device_id,
        host_pubkey: host_agent_keys.host_pubkey.to_string(),
        maybe_sys_pubkey: host_agent_keys.local_sys_pubkey.clone(),
        nonce: TextNonce::new().to_string(),
    };
    let payload_bytes = serde_json::to_vec(&payload)?;
    let signature = host_agent_keys.host_sign(&payload_bytes)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("X-Signature"),
        HeaderValue::from_str(&signature)?,
    );

    println!(
        "About to send out the {}.{} message",
        AUTH_SRV_SUBJ, VALIDATE_AUTH_SUBJECT
    );
    let response_msg = match auth_guard_client
        .request_with_headers(
            format!("{}.{}", AUTH_SRV_SUBJ, VALIDATE_AUTH_SUBJECT),
            headers,
            payload_bytes.into(),
        )
        .await
    {
        Ok(msg) => msg,
        Err(e) => {
            log::error!("{:#?}", e);
            if let RequestErrorKind::TimedOut = e.kind() {
                let unauthenticated_user_inventory_subject =
                    format!("INVENTORY.{}.unauthenticated", host_agent_keys.host_pubkey);
                let diganostics = HoloInventory::from_host();
                let payload_bytes = serde_json::to_vec(&diganostics)?;
                if (auth_guard_client
                    .publish(
                        unauthenticated_user_inventory_subject.to_string(),
                        payload_bytes.into(),
                    )
                    .await)
                    .is_ok()
                {
                    return Ok((host_agent_keys, auth_guard_client));
                }
            }
            return Err(async_nats::Error::from(e));
        }
    };

    println!(
        "Received AUTH response: {:#?}",
        serde_json::from_slice::<AuthJWTResult>(&response_msg.payload)
            .expect("failed to serde_json deserialize msg response")
    );

    if let Ok(auth_response) = serde_json::from_slice::<AuthJWTResult>(&response_msg.payload) {
        match auth_response.status {
            AuthState::Authorized => {
                host_agent_keys = host_agent_keys
                    .save_host_creds(auth_response.host_jwt, auth_response.sys_jwt)
                    .await?;
            }
            _ => {
                log::error!("got unexpected AUTH State : {:?}", auth_response);
            }
        }
    };

    Ok((host_agent_keys, auth_guard_client))
}
