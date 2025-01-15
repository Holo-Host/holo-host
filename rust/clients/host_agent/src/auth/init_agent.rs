/*
 This client is associated with the:
- ADMIN account
- noauth user

...once this the host and hoster are validated, this client should close and the hpos manager should spin up.

// This client is responsible for:
1. generating new key / re-using the user key from provided file
2. calling the auth service to:
  - verify host/hoster via `auth/start_hub_handshake` call
  - get hub operator jwt and hub sys account jwt via `auth/start_hub_handshake`
  - send "nkey" version of pubkey as file to hub via via `auth/end_hub_handshake`
  - get user jwt from hub via `auth/save_`
3. create user creds file with file path
4. instantiate the leaf server via the leaf-server struct/service
*/

use super::utils;
use anyhow::{anyhow, Result};
use async_nats::Message;
use authentication::{AuthApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::{sync::Arc, time::Duration};
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{self, EndpointType},
};

pub const HOST_INIT_CLIENT_NAME: &str = "Host Auth";
pub const HOST_INIT_CLIENT_INBOX_PREFIX: &str = "_host_auth_inbox";

const USER_CREDENTIALS_PATH: &str = "./user_creds";

pub async fn run() -> Result<(String, String), async_nats::Error> {
    log::info!("Host Auth Client: Connecting to server...");

    // ==================== NATS Setup ====================
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

    // ==================== DB Setup ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;

    // Generate the Auth API with access to db
    let auth_api = AuthApi::new(&client).await?;

    // ==================== Report Host to Orchestator ====================

    // Discover the server Node ID via INFO response
    let server_node_id = host_auth_client.get_server_info().server_id;
    log::trace!(
        "Host Auth Client: Retrieved Node ID: {}",
        server_node_id
    );

    // Publish a message with the Node ID as part of the subject
    let publish_options = nats_js_client::SendRequest {
        subject: format!("HPOS.init.{}", server_node_id),
        msg_id: format!("hpos_init_mid_{}", rand::random::<u8>()),
        data: b"Host Auth Connected!".to_vec(),
    };

    match host_auth_client
        .publish(&publish_options)
        .await
    {
        Ok(_r) => {
            log::trace!("Host Auth Client: Node ID published.");
        }
        Err(_e) => {}
    };

    // ==================== API ENDPOINTS ====================
    // Register Auth Streams for Orchestrator to consume and proceess
    // NB: The subjects below are published by the Orchestrator

    // Call auth service and perform auth handshake
    let auth_service = host_auth_client
        .get_js_service(AUTH_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Auth Service. Unable to spin up Orchestrator Auth Client."
        ))?;

    // i. register `save_hub_auth` consumer
    // ii. register `save_user_file` consumer
    // iii. send req for `` /eg:`start_hub_handshake`
    // iv. THEN (on get resp from start_handshake) `send_user_pubkey`

    // 1. make req (with agent key & email & nonce in payload, & sig in headers)
    // to receive_handhake_request
    // then await the reply (which should include the hub jwts)

    // 2. make req (with agent key as payload)
    // to add_user_pubkey
    // then await the reply (which should include the user jwt)

    // Register save service for hub auth files (operator and sys)
    auth_service
        .add_local_consumer::<authentication::types::ApiResult>(
            "save_hub_auth",
            "save_hub_auth",
            EndpointType::Async(auth_api.call(|api: AuthApi, msg: Arc<Message>| {
                async move {
                    api.save_hub_jwts(msg).await
                }
            })),
            None,
        )
        .await?;
        
    // Register save service for signed user jwt file
    auth_service
       .add_local_consumer::<authentication::types::ApiResult>(
            "save_user_jwt",
            "end_hub_handshake",
            EndpointType::Async(auth_api.call(|api: AuthApi, msg: Arc<Message>| {
                async move {
                    api.save_user_jwt(msg, USER_CREDENTIALS_PATH).await
                }
            })),
            None,
        )
        .await?;

    // Send the request (with payload) for the hub auth files and await a response
    // match client.request(subject, payload.into()).await {
    //     Ok(response) => {
    //         let response_str = String::from_utf8_lossy(&response.payload);
    //         println!("Received response: {}", response_str);
    //     }
    //     Err(e) => {
    //         println!("Failed to get a response: {}", e);
    //     }
    // }
    let req_hub_files_options = nats_js_client::SendRequest {
        subject: format!("HPOS.init.{}", server_node_id),
        msg_id: format!("hpos_init_mid_{}", rand::random::<u8>()),
        data: b"Host Auth Connected!".to_vec(),
    };
    host_auth_client.publish(&req_hub_files_options).await?;

    // ...upon the reply to the above, do the following: publish user pubkey file
    let _send_user_pubkey_publish_options = nats_js_client::SendRequest {
        subject: format!("HPOS.init.{}", server_node_id),
        msg_id: format!("hpos_init_mid_{}", rand::random::<u8>()),
        data: b"Host Auth Connected!".to_vec(),
    };
    // host_auth_client.publish(send_user_pubkey_publish_options);
    utils::chunk_file_and_publish(&host_auth_client.js, "subject", "file_path").await?;

    // 5. Generate user creds file
    let user_creds_path = utils::generate_creds_file();

    // 6. Close and drain internal buffer before exiting to make sure all messages are sent
    host_auth_client.close().await?;

    Ok(("host_pubkey_placeholder".to_string(), user_creds_path))
}
