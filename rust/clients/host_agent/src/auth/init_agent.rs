/*
This client is associated with the:
    - AUTH account
    - noauth user

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

use super::utils as local_utils;
use anyhow::{anyhow, Result};
use nkeys::KeyPair;
use std::str::FromStr;
use async_nats::{HeaderMap, HeaderName, HeaderValue, Message};
use authentication::{types::{AuthServiceSubjects, AuthRequestPayload, AuthApiResult}, AuthServiceApi, host_api::HostAuthApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION};
use core::option::Option::{None, Some};
use std::{collections::HashMap, sync::Arc, time::Duration};
use textnonce::TextNonce;
use util_libs::{
    js_stream_service::{JsServiceParamsPartial, ResponseSubjectsGenerator},
    nats_js_client::{self, EndpointType},
};

pub const HOST_INIT_CLIENT_NAME: &str = "Host Auth";
pub const HOST_INIT_CLIENT_INBOX_PREFIX: &str = "_host_auth_inbox";

pub fn create_callback_subject_to_orchestrator(sub_subject_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |_: HashMap<String, String>| -> Vec<String> {
        vec![format!("{}", sub_subject_name)]
    })
}

pub async fn run() -> Result<String, async_nats::Error> {
    log::info!("Host Auth Client: Connecting to server...");
    // ==================== Setup NATS ============================================================
    // Connect to Nats server
    let nats_url = nats_js_client::get_nats_url();
    let event_listeners = nats_js_client::get_event_listeners();

    // Setup JS Stream Service
    let auth_stream_service_params = JsServiceParamsPartial {
        name: AUTH_SRV_NAME.to_string(),
        description: AUTH_SRV_DESC.to_string(),
        version: AUTH_SRV_VERSION.to_string(),
        service_subject: AUTH_SRV_SUBJ.to_string(),
    };

    let host_auth_client =
        nats_js_client::JsClient::new(nats_js_client::NewJsClientParams {
            nats_url,
            name: HOST_INIT_CLIENT_NAME.to_string(),
            inbox_prefix: HOST_INIT_CLIENT_INBOX_PREFIX.to_string(),
            service_params: vec![auth_stream_service_params],
            credentials_path: None,
            opts: vec![nats_js_client::with_event_listeners(event_listeners)],
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
        })
        .await?;
    
    // ==================== Report Host to Orchestator ============================================
    // Generate Host Pubkey && Fetch Hoster Pubkey (from config)..
    // NB: This nkey keypair is a `ed25519_dalek::VerifyingKey` that is `BASE_32` encoded and returned as a String.
    let host_user_keys = KeyPair::new_user();
    let host_pubkey = host_user_keys.public_key();
    
    // Discover the server Node ID via INFO response
    let server_node_id = host_auth_client.get_server_info().server_id;
    log::trace!(
        "Host Auth Client: Retrieved Node ID: {}",
        server_node_id
    );
    
    // Publish a message with the Node ID as part of the subject
    let publish_options = nats_js_client::PublishInfo {
        subject: format!("HPOS.init.{}", server_node_id),
        msg_id: format!("hpos_init_mid_{}", rand::random::<u8>()),
        data: b"Host Auth Connected!".to_vec(),
        headers: None
    };
    
    match host_auth_client
    .publish(publish_options)
    .await
    {
        Ok(_r) => {
            log::trace!("Host Auth Client: Node ID published.");
        }
        Err(_e) => {}
    };
    
    // ==================== Setup API & Register Endpoints ===============================================
    // Generate the Auth API with access to db
    let auth_api = HostAuthApi::default();
    
    // Register Auth Streams for Orchestrator to consume and proceess
    // NB: The subjects below are published by the Orchestrator
    
    let auth_p1_subject = serde_json::to_string(&AuthServiceSubjects::HandleHandshakeP1)?;
    let auth_p2_subject = serde_json::to_string(&AuthServiceSubjects::HandleHandshakeP2)?;
    let auth_end_subject = serde_json::to_string(&AuthServiceSubjects::EndHandshake)?;

    // Call auth service and perform auth handshake
    let auth_service = host_auth_client
        .get_js_service(AUTH_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Auth Service. Unable to spin up Orchestrator Auth Client."
        ))?;

    // Register save service for hub auth files (operator and sys)
    auth_service
        .add_consumer::<AuthApiResult>(
            "save_hub_jwts", // consumer name
            &format!("{}.{}", host_pubkey, auth_p1_subject), // consumer stream subj
            EndpointType::Async(auth_api.call(|api: HostAuthApi, msg: Arc<Message>| {
                async move {
                    api.save_hub_jwts(msg).await
                }
            })),
            Some(create_callback_subject_to_orchestrator(auth_p2_subject)),
        )
        .await?;
        
    // Register save service for signed user jwt file
    auth_service
       .add_consumer::<AuthApiResult>(
            "save_user_jwt", // consumer name
            &format!("{}.{}", host_pubkey, auth_end_subject), // consumer stream subj
            EndpointType::Async(auth_api.call(|api: HostAuthApi, msg: Arc<Message>| {
                async move {
                    api.save_user_jwt(msg, &local_utils::get_host_credentials_path()).await
                }
            })),
            None,
        )
        .await?;

    // ==================== Publish Initial Auth Req =============================================
    // Initialize auth handshake with Orchestrator
    // by calling `AUTH.start_handshake` on the Auth Service
    let payload = AuthRequestPayload {
        host_pubkey: host_pubkey.clone(),
        email: "config.test.email@holo.host".to_string(),
        hoster_pubkey: "test_pubkey_from_config".to_string(),
        nonce: TextNonce::new().to_string()
    };

    let payload_bytes = serde_json::to_vec(&payload)?;
    let signature: Vec<u8> = host_user_keys.sign(&payload_bytes)?;

    let mut headers = HeaderMap::new();
    headers.insert(HeaderName::from_static("X-Signature"), HeaderValue::from_str(&format!("{:?}",signature))?);

    let publish_info = nats_js_client::PublishInfo {
        subject: "AUTH.start_handshake".to_string(),
        msg_id: format!("id={}", rand::random::<u8>()),
        data: payload_bytes,
        headers: Some(headers)
    };
    host_auth_client
        .publish(publish_info)
        .await?;

    log::trace!(
        "Init Host Agent Service is running. Waiting for requests..."
    );

    // ==================== Wait for Host Creds File & Safely Exit Auth Client ==================
    // Register FILE WATCHER and WAIT FOR the Host Creds File to exist
    // authentication::utils::get_file_path_buf(&host_creds_path).try_exists()?;

    // Close and drain internal buffer before exiting to make sure all messages are sent
    host_auth_client.close().await?;

    Ok(host_pubkey)
}
