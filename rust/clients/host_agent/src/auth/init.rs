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

use crate::{auth::config::HosterConfig, keys::Keys};
use anyhow::Result;
use std::str::FromStr;
use async_nats::{HeaderMap, HeaderName, HeaderValue};
use authentication::{
    types::{AuthApiResult, AuthRequestPayload, AuthGuardPayload}, utils::{handle_internal_err, write_file} // , AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION
};
use std::time::Duration;
use textnonce::TextNonce;
use util_libs:: nats_js_client::{self, get_nats_creds_by_nsc, get_file_path_buf, Credentials};

pub const HOST_AUTH_CLIENT_NAME: &str = "Host Auth";
pub const HOST_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX";

pub async fn run(mut host_agent_keys: Keys) -> Result<Keys, async_nats::Error> {
    log::info!("Host Auth Client: Connecting to server...");

    // ==================== Fetch Config File & Call NATS AuthCallout Service to Authenticate Host =============================================
    // Fetch Hoster Pubkey and email (from config)
    let config = HosterConfig::new().await?;

    let secret_token = TextNonce::new().to_string();
    let unique_inbox = format!("{}.{}", HOST_AUTH_CLIENT_INBOX_PREFIX, secret_token);
    println!(">>> unique_inbox : {}", unique_inbox);
    let user_unique_auth_subject = format!("AUTH.{}.>", secret_token);
    println!(">>> user_unique_auth_subject : {}", user_unique_auth_subject);

    let guard_payload = AuthGuardPayload {
        host_pubkey: host_agent_keys.host_pubkey,
        email: config.email,
        hoster_pubkey: config.hc_pubkey,
        nonce: secret_token
    };

    let user_auth_json = serde_json::to_string(&guard_payload).expect("Failed to serialize `UserAuthData` into json string");
    let user_auth_token = crate::utils::json_to_base64(&user_auth_json).expect("Failed to encode user token");

    // Connect to Nats server as auth guard and call NATS AuthCallout
    let nats_url = nats_js_client::get_nats_url();
    let event_listeners = nats_js_client::get_event_listeners();
    let auth_guard_creds = Credentials::Path(get_file_path_buf(&get_nats_creds_by_nsc("HOLO", "AUTH", "auth_guard")));

    let auth_guard_client = async_nats::ConnectOptions::with_credentials(user_creds)
        .expect("Failed to parse static creds")
        .token(user_auth_token)
        .custom_inbox_prefix(user_unique_inbox)
        .connect("nats://localhost:4222")
        .await?;

    println!("User connected to server on port {}.  Connection State: {:#?}", auth_guard_client.server_info().port, auth_guard_client.connection_state());

    let server_node_id = auth_guard_client.server_info().server_id;
    log::trace!(
        "Host Auth Client: Retrieved Node ID: {}",
        server_node_id
    );

    // ==================== Handle Authenication Results ============================================================
    let mut auth_inbox_msgs = auth_guard_client.subscribe(user_unique_inbox).await.unwrap();

    tokio::spawn({
        let auth_inbox_msgs_clone = auth_inbox_msgs.clone();
        async move {
            while let Some(msg) = auth_inbox_msgs_clone.next().await {
                println!("got an AUTH INBOX msg: {:?}", std::str::from_utf8(&msg.clone()).expect("failed to deserialize msg AUTH Inbox msg"));
                if let AuthApiResult(auth_response) = serde_json::from_slice(msg) {
                    host_agent_keys = crate::utils::save_host_creds(host_agent_keys, auth_response.host_jwt, auth_response.sys_jwt);
                    if let Some(reply) = msg.reply {
                        // Publish the Awk resp to the Orchestrator... (JS)
                    }
                    break;
                };
            }
        }
    });

    let payload = AuthRequestPayload {
        host_pubkey: host_agent_keys.host_pubkey,
        sys_pubkey: host_agent_keys.local_sys_pubkey,
        nonce: secret_token
    };

    let payload_bytes = serde_json::to_vec(&payload)?;
    let signature: Vec<u8> = host_user_keys.sign(&payload_bytes)?;
    let mut headers = HeaderMap::new();
    headers.insert(HeaderName::from_static("X-Signature"), HeaderValue::from_str(&format!("{:?}",signature))?);

    // let publish_info = nats_js_client::PublishInfo {
    //     subject: user_unique_auth_subject,
    //     msg_id: format!("id={}", rand::random::<u8>()),
    //     data: payload_bytes,
    //     headers: Some(headers)
    // };
    
    println!(format!("About to send out the {user_unique_auth_subject} message"));
    let response = auth_guard_client.request_with_headers(user_unique_auth_subject, headers, payload_bytes).await.expect(&format!("Failed to make {user_unique_auth_subject} request"));
    println!("got an AUTH response: {:?}", std::str::from_utf8(&response.payload).expect("failed to deserialize msg response"));
    
    match serde_json::from_slice::<AuthApiResult>(&response.payload) {
        Ok(r) => {
            host_agent_keys = crate::utils::save_host_creds(host_agent_keys, auth_response.host_jwt, auth_response.sys_jwt);
            
            if let Some(reply) = msg.reply {
                // Publish the Awk resp to the Orchestrator... (JS)
            }
        },
        Err(e) => {
            // TODO:
            // Check to see if error is due to auth error.. if so then try to publish to Diagnostics Subject at regular intervals
            // for a set period of time, then exit loop and initiate auth connection...
            let payload = "hpos-hal.info()".as_bytes();
            let mut auth_inbox_msgs = auth_guard_client.publish("DIAGNOSTICS.ERROR", payload).await.unwrap();
        }
    };

    // Close and drain internal buffer before exiting to make sure all messages are sent
    auth_guard_client.close().await?;

    log::trace!(
        "host_agent_keys: {}", host_agent_keys
    );

    Ok(host_agent_keys)
}


