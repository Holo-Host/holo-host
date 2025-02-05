/*
This client is associated with the:
    - ADMIN account
    - orchestrator user

This client is responsible for:
    - initalizing connection and handling interface with db
    - registering with the host auth service to:
        - handling auth requests by:
            - validating user signature
            - validating hoster pubkey
            - validating hoster email
            - bidirectionally pairing hoster and host
            - interfacing with hub nsc resolver and hub credential files
            - adding user to hub
            - creating signed jwt for user
            - adding user jwt file to user collection (with ttl)
    - keeping service running until explicitly cancelled out
*/

use anyhow::{anyhow, Result};
use async_nats::Message;
use authentication::{
    self,
    types::{self, AuthApiResult},
    AuthServiceApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION,
};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use nkeys::KeyPair;
use std::{collections::HashMap, sync::Arc, time::Duration};
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::{JsServiceParamsPartial, ResponseSubjectsGenerator},
    nats_js_client::{
        get_event_listeners, get_file_path_buf, get_nats_url, with_event_listeners, Credentials,
        EndpointType, JsClient, NewJsClientParams,
    },
};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Agent";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_auth_inbox_orchestrator";

pub async fn run() -> Result<(), async_nats::Error> {
    let admin_account_creds_path = get_file_path_buf("test_admin.creds");
    println!(
        " >>>> admin_account_creds_path: {:#?} ",
        admin_account_creds_path
    );

    // Root Keypair associated with AUTH account
    let root_account_keypair = Arc::new(KeyPair::from_seed("<TEST_SK_SEED>")?);
    let root_account_pubkey = root_account_keypair.public_key().clone();

    // AUTH Account Signing Keypair associated with the `auth` user
    let signing_account_keypair = Arc::new(KeyPair::from_seed("<TEST_SK_SEED>")?);
    let signing_account_pubkey = signing_account_keypair.public_key().clone();
    println!(
        ">>>>>>>>> signing_account pubkey: {:?}",
        signing_account_pubkey
    );

    // ==================== Setup NATS ====================
    // Setup JS Stream Service
    let auth_stream_service_params = JsServiceParamsPartial {
        name: AUTH_SRV_NAME.to_string(),
        description: AUTH_SRV_DESC.to_string(),
        version: AUTH_SRV_VERSION.to_string(),
        service_subject: AUTH_SRV_SUBJ.to_string(),
    };

    let orchestrator_auth_client = JsClient::new(NewJsClientParams {
        nats_url: get_nats_url(),
        name: ORCHESTRATOR_AUTH_CLIENT_NAME.to_string(),
        inbox_prefix: ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX.to_string(),
        service_params: vec![auth_stream_service_params],
        credentials: Some(Credentials::Path(admin_account_creds_path)),
        listeners: vec![with_event_listeners(get_event_listeners())],
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
    let auth_api = AuthServiceApi::new(&client).await?;

    // Register Auth Stream for Orchestrator to consume and process
    let auth_service = orchestrator_auth_client
        .get_js_service(AUTH_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Auth Service. Unable to spin up Orchestrator Auth Client."
        ))?;

    auth_service
        .add_consumer::<AuthApiResult>(
            "auth_callout",
            types::AUTH_CALLOUT_SUBJECT, // consumer stream subj
            EndpointType::Async(auth_api.call({
                move |api: AuthServiceApi, msg: Arc<Message>| {
                    let signing_account_kp = Arc::clone(&signing_account_keypair);
                    let signing_account_pk = signing_account_pubkey.clone();
                    let root_account_kp = Arc::clone(&root_account_keypair);
                    let root_account_pk = root_account_pubkey.clone();

                    async move {
                        api.handle_auth_callout(
                            msg,
                            signing_account_kp,
                            signing_account_pk,
                            root_account_kp,
                            root_account_pk,
                        )
                        .await
                    }
                }
            })),
            None,
        )
        .await?;

    auth_service
        .add_consumer::<AuthApiResult>(
            "authorize_host_and_sys",
            types::AUTHORIZE_SUBJECT, // consumer stream subj
            EndpointType::Async(auth_api.call(
                |api: AuthServiceApi, msg: Arc<Message>| async move {
                    api.handle_handshake_request(msg).await
                },
            )),
            Some(create_callback_subject_to_host("host_pubkey".to_string())),
        )
        .await?;

    log::trace!("Orchestrator Auth Service is running. Waiting for requests...");

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_auth_client.close().await?;

    Ok(())
}

pub fn create_callback_subject_to_host(tag_name: String) -> ResponseSubjectsGenerator {
    Arc::new(move |tag_map: HashMap<String, String>| -> Vec<String> {
        if let Some(tag) = tag_map.get(&tag_name) {
            return vec![format!("AUTH.{}", tag)];
        }
        log::error!("Auth Error: Failed to find {}. Unable to send orchestrator response to hosting agent for subject 'AUTH.validate'. Fwding response to `AUTH.ERROR.INBOX`.", tag_name);
        vec!["AUTH.ERROR.INBOX".to_string()]
    })
}
