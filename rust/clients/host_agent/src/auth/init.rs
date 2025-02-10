/*
This client is associated with the:
    - ADMIN account
    - auth guard user

Nb: Once the host and hoster are validated, and the host creds file is created,
...this client should close and the hostd workload manager should spin up.

This client is responsible for:
    - generating new key for host / and accessing hoster key from provided config file
    - registering with the host auth service to:
        - get hub operator jwt and hub sys account jwt
        - send "nkey" version of host pubkey as file to hub
        - get user jwt from hub and create user creds file with provided file path
    - publishing to `auth.start` to initilize the auth handshake and validate the host/hoster
    - returning the host pubkey and closing client cleanly
*/

use super::utils::json_to_base64;
use crate::{
    auth::config::HosterConfig,
    keys::{AuthCredType, Keys},
};
use anyhow::Result;
use async_nats::{HeaderMap, HeaderName, HeaderValue};
use authentication::types::{AuthApiResult, AuthGuardPayload, AuthJWTPayload, AuthResult}; // , AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION
use futures::StreamExt;
use hpos_hal::inventory::HoloInventory;
use std::str::FromStr;
use textnonce::TextNonce;
use util_libs::nats_js_client;

// pub const HOST_AUTH_CLIENT_NAME: &str = "Host Auth";
pub const HOST_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX";

pub async fn run(
    mut host_agent_keys: Keys,
) -> Result<(Keys, async_nats::Client), async_nats::Error> {
    log::info!("Host Auth Client: Connecting to server...");

    // ==================== Fetch Config File & Call NATS AuthCallout Service to Authenticate Host & Hoster =============================================
    let nonce = TextNonce::new().to_string();
    let unique_inbox = &format!(
        "{}_{}",
        HOST_AUTH_CLIENT_INBOX_PREFIX, host_agent_keys.host_pubkey
    );
    println!(">>> unique_inbox : {}", unique_inbox);
    let user_unique_auth_subject = &format!("AUTH.{}.>", host_agent_keys.host_pubkey);
    println!(
        ">>> user_unique_auth_subject : {}",
        user_unique_auth_subject
    );

    // Fetch Hoster Pubkey and email (from config)
    let mut auth_guard_payload = AuthGuardPayload::default();
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
    let user_creds = if let AuthCredType::Guard(creds) = host_agent_keys.creds.clone() {
        creds
    } else {
        return Err(async_nats::Error::from(
            "Failed to locate Auth Guard credentials",
        ));
    };

    // Connect to Nats server as auth guard and call NATS AuthCallout
    let nats_url = nats_js_client::get_nats_url();
    let auth_guard_client =
        async_nats::ConnectOptions::with_credentials(&user_creds.to_string_lossy())?
            .token(user_auth_token)
            .custom_inbox_prefix(unique_inbox.to_string())
            .connect(nats_url)
            .await?;

    println!(
        "User connected to server on port {}.  Connection State: {:#?}",
        auth_guard_client.server_info().port,
        auth_guard_client.connection_state()
    );

    let server_node_id = auth_guard_client.server_info().server_id;
    log::trace!("Host Auth Client: Retrieved Node ID: {}", server_node_id);

    // ==================== Handle Host User and SYS Authoriation ============================================================
    let auth_guard_client_clone = auth_guard_client.clone();
    tokio::spawn({
        let mut auth_inbox_msgs = auth_guard_client_clone
            .subscribe(unique_inbox.to_string())
            .await?;
        async move {
            while let Some(msg) = auth_inbox_msgs.next().await {
                println!("got an AUTH INBOX msg: {:?}", &msg);
            }
        }
    });

    let payload = AuthJWTPayload {
        host_pubkey: host_agent_keys.host_pubkey.to_string(),
        maybe_sys_pubkey: host_agent_keys.local_sys_pubkey.clone(),
        nonce: TextNonce::new().to_string(),
    };

    let payload_bytes = serde_json::to_vec(&payload)?;
    let signature = host_agent_keys.host_sign(&payload_bytes)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("X-Signature"),
        HeaderValue::from_str(&format!("{:?}", signature.as_bytes()))?,
    );

    println!("About to send out the {} message", user_unique_auth_subject);
    let response = auth_guard_client
        .request_with_headers(
            user_unique_auth_subject.to_string(),
            headers,
            payload_bytes.into(),
        )
        .await?;

    println!(
        "got an AUTH response: {:?}",
        std::str::from_utf8(&response.payload).expect("failed to deserialize msg response")
    );

    match serde_json::from_slice::<AuthApiResult>(&response.payload) {
        Ok(auth_response) => match auth_response.result {
            AuthResult::Authorization(r) => {
                host_agent_keys = host_agent_keys
                    .save_host_creds(r.host_jwt, r.sys_jwt)
                    .await?;

                if let Some(_reply) = response.reply {
                    // Publish the Awk resp to the Orchestrator... (JS)
                }
            }
            _ => {
                log::error!("got unexpected AUTH RESPONSE : {:?}", auth_response);
            }
        },
        Err(e) => {
            // TODO: Check to see if error is due to auth error.. if so then try to publish to Diagnostics Subject to ensure has correct permissions
            println!("got an AUTH RES ERROR: {:?}", e);

            let unauthenticated_user_diagnostics_subject = format!(
                "DIAGNOSTICS.unauthenticated.{}",
                host_agent_keys.host_pubkey
            );
            let diganostics = HoloInventory::from_host();
            let payload_bytes = serde_json::to_vec(&diganostics)?;
            auth_guard_client
                .publish(
                    unauthenticated_user_diagnostics_subject,
                    payload_bytes.into(),
                )
                .await?;
        }
    };

    log::trace!("host_agent_keys: {:#?}", host_agent_keys);

    Ok((host_agent_keys, auth_guard_client))
}
