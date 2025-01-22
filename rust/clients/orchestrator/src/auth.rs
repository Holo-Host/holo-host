/*
This client is associated with the:
    - AUTH account
    - orchestrator user

This client is responsible for:
    - initalizing connection and handling interface with db
    - registering with the host auth service to:
        - handling inital auth requests
            - validating user signature
            - validating hoster pubkey
            - validating hoster email
            - bidirectionally pairing hoster and host
            - sending hub jwt files back to user (once validated) 
        - handling request to add user pubkey and generate signed user jwt 
            - interfacing with hub nsc resolver and hub credential files
            - sending user jwt file back to user
    - keeping service running until explicitly cancelled out
*/

use crate::utils as local_utils;

use anyhow::{anyhow, Result};
use std::{collections::HashMap, sync::Arc, time::Duration};
// use std::process::Command;
use async_nats::Message;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use authentication::{self, types::{AuthServiceSubjects, AuthApiResult}, AuthServiceApi, orchestrator_api::OrchestratorAuthApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION};
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::{JsServiceParamsPartial, ResponseSubjectsGenerator},
    nats_js_client::{self, EndpointType, JsClient, NewJsClientParams},
};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Agent";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_orchestrator_auth_inbox";

pub fn create_callback_subject_to_host(tag_name: String, sub_subject_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |tag_map: HashMap<String, String>| -> Vec<String> {
        if let Some(tag) = tag_map.get(&tag_name) {
            return vec![format!("{}.{}", tag, &sub_subject_name)];
        }
        log::error!("Auth Error: Failed to find {}. Unable to send orchestrator response to hosting agent for subject {}. Fwding response to `AUTH.ERROR.INBOX`.", tag_name, sub_subject_name);
        vec!["AUTH.ERROR.INBOX".to_string()]
    })
}

pub async fn run() -> Result<(), async_nats::Error> {
    // ==================== Setup NATS ====================
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

    // ==================== Setup DB ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;
    
    // ==================== Setup API & Register Endpoints ====================
    // Generate the Auth API with access to db
    let auth_api = OrchestratorAuthApi::new(&client).await?;
    
    // Register Auth Streams for Orchestrator to consume and proceess
    // NB: The subjects below are published by the Host Agent
    let auth_start_subject = serde_json::to_string(&AuthServiceSubjects::StartHandshake)?;
    let auth_p1_subject = serde_json::to_string(&AuthServiceSubjects::HandleHandshakeP1)?;
    let auth_p2_subject = serde_json::to_string(&AuthServiceSubjects::HandleHandshakeP2)?;
    let auth_end_subject = serde_json::to_string(&AuthServiceSubjects::EndHandshake)?;

    let auth_service = orchestrator_auth_client
        .get_js_service(AUTH_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Auth Service. Unable to spin up Orchestrator Auth Client."
        ))?;    
    
    auth_service
        .add_consumer::<AuthApiResult>(
            "start_handshake", // consumer name
            &auth_start_subject, // consumer stream subj
            EndpointType::Async(auth_api.call(|api: OrchestratorAuthApi, msg: Arc<Message>| {
                async move {
                    api.handle_handshake_request(msg, &local_utils::get_orchestrator_credentials_dir_path()).await
                }
            })),
            Some(create_callback_subject_to_host("host_pubkey".to_string(), auth_p1_subject)),
        )
        .await?;
        
    auth_service
        .add_consumer::<AuthApiResult>(
            "add_user_pubkey", // consumer name
            &auth_p2_subject, // consumer stream subj
            EndpointType::Async(auth_api.call(|api: OrchestratorAuthApi, msg: Arc<Message>| {
                async move {
                    api.add_user_nkey(msg, &local_utils::get_orchestrator_credentials_dir_path()).await
                }
            })),
            Some(create_callback_subject_to_host("host_pubkey".to_string(), auth_end_subject)),
        )
        .await?;

    log::trace!(
        "Orchestrator Auth Service is running. Waiting for requests..."
    );

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_auth_client.close().await?;

    Ok(())
}
