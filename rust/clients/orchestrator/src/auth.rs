/*
 This client is associated with the:
- auth account
- orchestrator user

// This client is responsible for:
*/

use crate::utils;

use anyhow::{anyhow, Result};
use std::{sync::Arc, time::Duration};
use async_nats::Message;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use authentication::{self, AuthApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION};
use std::process::Command;
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsServiceParamsPartial,
    nats_js_client::{self, EndpointType, JsClient, NewJsClientParams},
};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Agent";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_orchestrator_auth_inbox";

pub async fn run() -> Result<(), async_nats::Error> {
    // ==================== NATS Setup ====================
    let nats_url = nats_js_client::get_nats_url();
    let event_listeners = nats_js_client::get_event_listeners();

    // Setup JS Stream Service
    let auth_stream_service_params = JsServiceParamsPartial {
        name: AUTH_SRV_NAME.to_string(),
        description: AUTH_SRV_DESC.to_string(),
        version: AUTH_SRV_VERSION.to_string(),
        service_subject: AUTH_SRV_SUBJ.to_string(),
    };
    
    let orchestrator_auth_client =
        JsClient::new(NewJsClientParams {
            nats_url,
            name: ORCHESTRATOR_AUTH_CLIENT_NAME.to_string(),
            inbox_prefix: ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX.to_string(),
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

    // ==================== API ENDPOINTS ====================
    // Register Auth Streams for Orchestrator to consume and proceess
    // NB: The subjects below are published by the Host Agent
    let auth_service = orchestrator_auth_client
        .get_js_service(AUTH_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Auth Service. Unable to spin up Orchestrator Auth Client."
        ))?;

    auth_service
        .add_local_consumer::<authentication::types::ApiResult>(
            "add_user_pubkey",
            "add_user_pubkey",
            EndpointType::Async(auth_api.call(|api: AuthApi, msg: Arc<Message>| {
                async move {
                    api.add_user_pubkey(msg).await
                }
            })),
            None,
        )
        .await?;

    log::trace!(
        "{} Service is running. Waiting for requests...",
        AUTH_SRV_NAME
    );


    let resolver_path = utils::get_resolver_path();

    let _auth_endpoint_subject = format!("AUTH.{}.file.transfer.JWT-User", "host_id_placeholder"); // endpoint_subject
 
    // Generate resolver file and create resolver file
    Command::new("nsc")
        .arg("generate")
        .arg("config")
        .arg("--nats-resolver")
        .arg("sys-account SYS")
        .arg("--force")
        .arg(format!("--config-file {}", resolver_path))
        .output()
        .expect("Failed to create resolver config file");

    // Push auth updates to hub server
    Command::new("nsc")
        .arg("push -A")
        .output()
        .expect("Failed to create resolver config file");

    // publish user jwt file
    let server_node_id = "server_node_id_placeholder";
    utils::chunk_file_and_publish(
        &orchestrator_auth_client,
        &format!("HPOS.init.{}", server_node_id),
        "placeholder_user_id / pubkey",
    )
    .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;
    log::warn!("CTRL+C detected. Please press CTRL+C again within 5 seconds to confirm exit...");
    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => log::warn!("Resuming service."),
        _ = tokio::signal::ctrl_c() => log::error!("Shutting down."),
    }

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_auth_client.close().await?;

    Ok(())
}
