use super::endpoints;
use crate::utils;
use anyhow::Result;
use authentication::{AuthApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::process::Command;
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::JsStreamService,
    nats_js_client::{self, EndpointType, JsClient},
};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Agent";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_orchestrator_auth_inbox";

pub async fn run() -> Result<(), async_nats::Error> {
    // ==================== NATS Setup ====================
    let nats_url = nats_js_client::get_nats_url();
    let creds_path = nats_js_client::get_nats_client_creds("HOLO", "ADMIN", "orchestrator");
    let event_listeners = nats_js_client::get_event_listeners();

    let orchestrator_auth_client =
        nats_js_client::DefaultJsClient::new(nats_js_client::NewDefaultJsClientParams {
            nats_url,
            name: ORCHESTRATOR_AUTH_CLIENT_NAME.to_string(),
            inbox_prefix: ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX.to_string(),
            opts: vec![nats_js_client::with_event_listeners(event_listeners)],
            credentials_path: Some(creds_path),
            ..Default::default()
        })
        .await?;

    // Create a new Jetstream Microservice
    let js_service = JsStreamService::new(
        orchestrator_auth_client.js.clone(),
        AUTH_SRV_NAME,
        AUTH_SRV_DESC,
        AUTH_SRV_VERSION,
        AUTH_SRV_SUBJ,
    )
    .await?;

    // ==================== DB Setup ====================

    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;

    // Generate the Workload API with access to db
    let auth_api = AuthApi::new(&client).await?;

    // ==================== API ENDPOINTS ====================
    // Register Workload Streams for Host Agent to consume
    // (subjects should be published by orchestrator or nats-db-connector)

    let auth_endpoint_subject = format!("AUTH.{}.file.transfer.JWT-User", "host_id_placeholder"); // endpoint_subject

    js_service
        .add_local_consumer(
            "add_user_pubkey", // called from orchestrator (no -auth service)
            "add_user_pubkey",
            EndpointType::Async(endpoints::add_user_pubkey(&auth_api).await),
            None,
        )
        .await?;

    log::trace!(
        "{} Service is running. Waiting for requests...",
        AUTH_SRV_NAME
    );

    let resolver_path = utils::get_resolver_path();

    // Generate resolver file and create resolver file
    Command::new("nsc")
        .arg("generate")
        .arg("config")
        .arg("--nats-resolver")
        .arg("sys-account SYS")
        .arg("--force")
        .arg(format!("--config-file {}", resolver_path))
        .output()
        .expect("Failed to create resolver config file")
        .stdout;

    // Push auth updates to hub server
    Command::new("nsc")
        .arg("push -A")
        .output()
        .expect("Failed to create resolver config file")
        .stdout;

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
