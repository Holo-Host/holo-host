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
use async_nats::{jetstream::context::PublishErrorKind, HeaderMap, HeaderName, HeaderValue, RequestErrorKind};
use authentication::types::{AuthGuardPayload, AuthJWTPayload, AuthJWTResult, AuthResult, AuthState};
use std::time::Duration;
// use futures::StreamExt;
use util_libs::nats_js_client::{
    self, get_event_listeners, get_nats_url, with_event_listeners,
    Credentials, JsClient, NewJsClientParams,
};
use hpos_hal::inventory::HoloInventory;
use std::str::FromStr;
use textnonce::TextNonce;

pub const HOST_AUTH_CLIENT_NAME: &str = "Host Auth";
pub const HOST_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX";

pub async fn run(
    mut host_agent_keys: Keys,
) -> Result<(Keys, async_nats::Client), async_nats::Error> {
    log::info!("Host Auth Client: Connecting to server...");
    println!("Keys={:#?}", host_agent_keys);
    
    println!("inside init auth... 0");
    
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
    println!("inside init auth... 1");

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
    println!("auth_guard_payload={:#?}", auth_guard_payload);

    let user_auth_json = serde_json::to_string(&auth_guard_payload)?;
    let user_auth_token = json_to_base64(&user_auth_json)?;
    let user_creds_path = if let AuthCredType::Guard(creds) = host_agent_keys.creds.clone() {
        creds
    } else {
        return Err(async_nats::Error::from(
            "Failed to locate Auth Guard credentials",
        ));
    };
    println!("user_creds_path={:#?}", user_creds_path);

    let user_creds = "-----BEGIN NATS USER JWT-----
eyJ0eXAiOiJKV1QiLCJhbGciOiJlZDI1NTE5LW5rZXkifQ.eyJqdGkiOiI2MkVCSEhFR0M1RDIyU0lXR1hSU0paNEpWWkdWUk9FVUo3N1BYQ1BPNUU3UDRBTkVHV1RBIiwiaWF0IjoxNzM4NTI3NjgwLCJpc3MiOiJBRFQ2TUhSQUgzU0JXWFU1RlRHN0I2WklCU0VXV0UzMkJVNDJKTzRKRE8yV0VSVDZYTVpLRTYzUyIsIm5hbWUiOiJhdXRoLWd1YXJkIiwic3ViIjoiVUM1N1pETUtOSVhVWE9NNlRISE8zQjVVRUlWQ0JPM0hNRlUzSU5ESVZNTzVCSFZKR1k3R1hIM1UiLCJuYXRzIjp7InB1YiI6eyJkZW55IjpbIlx1MDAzZSJdfSwic3ViIjp7ImRlbnkiOlsiXHUwMDNlIl19LCJzdWJzIjotMSwiZGF0YSI6LTEsInBheWxvYWQiOi0xLCJpc3N1ZXJfYWNjb3VudCI6IkFBM1E3SFlQR01XUlhXMkZMSzVDQkRWUFlXRFIyN01OUFBPU09TN0lHVU9IQVkzTDRHTlJCTEo0IiwidHlwZSI6InVzZXIiLCJ2ZXJzaW9uIjoyfX0.REQSfDwGzuG0vWDDfHyZdpN-Ens3hhRF1-I-k5akDK9oT8kueW2OWX3lFlgBreNw5JsTgE0fjKDq942QRTygDg
------END NATS USER JWT------

************************* IMPORTANT *************************
NKEY Seed printed below can be used to sign and prove identity.
NKEYs are sensitive and should be treated as secrets.

-----BEGIN USER NKEY SEED-----
SUABBYL4YAGRRJDOMXP72EUDM4UOFOGJWVPKT6AB7UMNWU2TV4M4PMFXDE
------END USER NKEY SEED------

*************************************************************";

    // Connect to Nats server as auth guard and call NATS AuthCallout
    let nats_url = nats_js_client::get_nats_url();
    let auth_guard_client =
        async_nats::ConnectOptions::new()
            .name(HOST_AUTH_CLIENT_NAME.to_string())
            .custom_inbox_prefix(unique_inbox.to_string())
            .ping_interval(Duration::from_secs(10))
            .request_timeout(Some(Duration::from_secs(30)))
            .token(user_auth_token)
            // .credentials_file(&user_creds_path).await?
            .credentials(user_creds)?
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
    let auth_guard_client_clone = auth_guard_client.clone();

    // tokio::spawn({
    //     let mut auth_inbox_msgs = auth_guard_client_clone
    //         .subscribe(unique_inbox.to_string())
    //         .await?;
    //     async move {
    //         while let Some(msg) = auth_inbox_msgs.next().await {
    //             println!("got an AUTH INBOX msg: {:?}", &msg);
    //         }
    //     }
    // });

    let payload = AuthJWTPayload {
        host_pubkey: host_agent_keys.host_pubkey.to_string(),
        maybe_sys_pubkey: host_agent_keys.local_sys_pubkey.clone(),
        nonce: TextNonce::new().to_string(),
    };
    println!("inside init auth... 9");

    let payload_bytes = serde_json::to_vec(&payload)?;
    println!("inside init auth... 10");

    let signature = host_agent_keys.host_sign(&payload_bytes)?;
    println!("inside init auth... 11");
    println!(" >>> signature >>> {}", signature);
    println!(" >>> signature.as_bytes() >>> {:?}", signature.as_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("X-Signature"),
        HeaderValue::from_str(&signature)?,
    );
    
    println!("About to send out the {} message", "AUTH.validate");
    let response_msg = match auth_guard_client
        .request_with_headers(
            "AUTH.validate".to_string(),
            headers,
            payload_bytes.into()
        )
        .await {
            Ok(msg) => msg,
            Err(e) => {
                log::error!("{:#?}", e);
                if let RequestErrorKind::TimedOut = e.kind() {
                    println!("inside init auth... 13");

                    // TODO: Check to see if error is due to auth error.. if so then try to publish to Diagnostics Subject to ensure has correct permissions
                    println!("got an AUTH RES ERROR: {:?}", e);
        
                    let unauthenticated_user_diagnostics_subject = format!(
                        "DIAGNOSTICS.unauthenticated.{}",
                        host_agent_keys.host_pubkey
                    );
                    let diganostics = HoloInventory::from_host();
                    let payload_bytes = serde_json::to_vec(&diganostics)?;                  
            
                    if let Ok(_) = auth_guard_client
                        .publish( unauthenticated_user_diagnostics_subject.to_string(), payload_bytes.into())
                        .await {
                            return Ok((host_agent_keys, auth_guard_client));
                        }
                }
                return Err(async_nats::Error::from(e));
            }
        };

    println!(
        "got an AUTH response: {:?}",
        std::str::from_utf8(&response_msg.payload).expect("failed to deserialize msg response")
    );

    println!(
        "got an AUTH response: {:#?}",
        serde_json::from_slice::<AuthJWTResult>(&response_msg.payload).expect("failed to serde_json deserialize msg response")
    );

    if let Ok(auth_response) = serde_json::from_slice::<AuthJWTResult>(&response_msg.payload) {
        match auth_response.status {
            AuthState::Authorized => {
                println!("inside init auth... 13");

                host_agent_keys = host_agent_keys
                    .save_host_creds(auth_response.host_jwt, auth_response.sys_jwt)
                    .await?;

                if let Some(_reply) = response_msg.reply {
                    // Publish the Awk resp to the Orchestrator... (JS)
                }
            }
            _ => {
                println!("inside init auth... 13");
                log::error!("got unexpected AUTH State : {:?}", auth_response);
            }
        }
    };

    println!("inside init auth... 14");
    println!("host_agent_keys: {:#?}", host_agent_keys);
    Ok((host_agent_keys, auth_guard_client))
}
