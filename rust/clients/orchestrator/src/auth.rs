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

use anyhow::{anyhow, Context, Result};
use async_nats::Message;
use authentication::{
    self,
    types::{self, AuthApiResult},
    AuthServiceApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION,
};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use nkeys::KeyPair;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{collections::HashMap, sync::Arc, time::Duration};
use std::str::FromStr;
use util_libs::{
    db::mongodb::get_mongodb_url,
    js_stream_service::{JsServiceParamsPartial, ResponseSubjectsGenerator},
    nats_js_client::{
        get_event_listeners, get_nats_url, with_event_listeners, get_nats_creds_by_nsc, Credentials,
        EndpointType, JsClient, NewJsClientParams,
    },
};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Manager";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_auth_inbox_orchestrator";

pub async fn run() -> Result<(), async_nats::Error> {
    println!("inside auth... 0");

    // let admin_account_creds_path = PathBuf::from_str("/home/za/Documents/holo-v2/holo-host/rust/clients/orchestrator/src/tmp/test_admin.creds")?;
    let admin_account_creds_path = PathBuf::from_str(&get_nats_creds_by_nsc(
        "HOLO",
        "AUTH",
        "auth",
    ))?;
    println!(
        " >>>> admin_account_creds_path: {:#?} ",
        admin_account_creds_path
    );

    println!("inside auth... 1");

    // Root Keypair associated with AUTH account
    let root_account_key_path = std::env::var("ROOT_AUTH_NKEY_PATH")
        .context("Cannot read ROOT_AUTH_NKEY_PATH from env var")?;
    let root_account_keypair = Arc::new(
        try_read_keypair_from_file(PathBuf::from_str(&root_account_key_path.clone())?)?.ok_or_else(
            || {
                anyhow!(
                    "Root AUTH Account keypair not found at path {:?}",
                    root_account_key_path
                )
            },
        )?,
    );

    println!("inside auth... 2");

    // TODO: REMOVE
    // let root_account_keypair = Arc::new(KeyPair::from_seed(
    //     "<>",
    // )?);
    let root_account_pubkey = root_account_keypair.public_key().clone();
    println!("inside auth... 3");

    // AUTH Account Signing Keypair associated with the `auth` user
    let signing_account_key_path = std::env::var("SIGNING_AUTH_NKEY_PATH")
        .context("Cannot read SIGNING_AUTH_NKEY_PATH from env var")?;
    println!("inside auth... 4");

    let signing_account_keypair = Arc::new(
        try_read_keypair_from_file(PathBuf::from_str(&signing_account_key_path.clone())?)?
            .ok_or_else(|| {
                anyhow!(
                    "Signing AUTH Account keypair not found at path {:?}",
                    signing_account_key_path
                )
            })?,
    );
    println!("inside auth... 5");

    // TODO: REMOVE
    // let signing_account_keypair = Arc::new(KeyPair::from_seed(
    //     "<>",
    // )?);
    let signing_account_pubkey = signing_account_keypair.public_key().clone();
    println!(
        ">>>>>>>>> signing_account pubkey: {:?}",
        signing_account_pubkey
    );
    println!("inside auth... 6");


    // ==================== Setup NATS ====================
    // Setup JS Stream Service
    let auth_stream_service_params = JsServiceParamsPartial {
        name: AUTH_SRV_NAME.to_string(),
        description: AUTH_SRV_DESC.to_string(),
        version: AUTH_SRV_VERSION.to_string(),
        service_subject: AUTH_SRV_SUBJ.to_string(),
    };
    println!("inside auth... 7");
    let nats_url = get_nats_url();
    let nats_connect_timeout_secs: u64 = 180; 

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

    // let orchestrator_auth_client = tokio::select! {
    //     client = async {loop {
    //             let orchestrator_auth_client = JsClient::new(NewJsClientParams {
    //                 nats_url: nats_url.clone(),
    //                 name: ORCHESTRATOR_AUTH_CLIENT_NAME.to_string(),
    //                 inbox_prefix: ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX.to_string(),
    //                 service_params: vec![auth_stream_service_params.clone()],
    //                 credentials: Some(Credentials::Path(admin_account_creds_path.clone())),
    //                 listeners: vec![with_event_listeners(get_event_listeners())],
    //                 ping_interval: Some(Duration::from_secs(10)),
    //                 request_timeout: Some(Duration::from_secs(5)),
    //             })
    //             .await
    //             .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"));

    //             match orchestrator_auth_client {
    //                 Ok(client) => break client,
    //                 Err(e) => {
    //                     let duration = tokio::time::Duration::from_millis(100);
    //                     log::warn!("{}, retrying in {duration:?}", e);
    //                     tokio::time::sleep(duration).await;
    //                 }
    //             }
    //         }} => client,
    //     _ = {
    //         log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
    //         tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
    //      } => {
    //         return Err(format!("timed out waiting for NATS on {nats_url}").into());
    //     }
    // };

    println!("inside auth... 8");

    // ==================== Setup DB ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let client = MongoDBClient::with_options(client_options)?;
    println!("inside auth... 9");
    
    // ==================== Setup API & Register Endpoints ====================
    // Generate the Auth API with access to db
    let auth_api = AuthServiceApi::new(&client).await?;
    println!("inside auth... 10");

    // Register Auth Stream for Orchestrator to consume and process
    let auth_service = orchestrator_auth_client
        .get_js_service(AUTH_SRV_NAME.to_string())
        .await
        .ok_or(anyhow!(
            "Failed to locate Auth Service. Unable to spin up Orchestrator Auth Client."
        ))?;
    println!("inside auth... 11");

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
    println!("inside auth... 12");

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
    println!("inside auth... 13");

    println!("Orchestrator Auth Service is running. Waiting for requests...");

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    println!("inside auth... 14... closing");

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_auth_client.close().await?;
    println!("inside auth... 15... closed");

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

fn try_read_keypair_from_file(key_file_path: PathBuf) -> Result<Option<KeyPair>> {
    match try_read_from_file(key_file_path)? {
        Some(kps) => Ok(Some(KeyPair::from_seed(&kps)?)),
        None => Ok(None),
    }
}

fn try_read_from_file(file_path: PathBuf) -> Result<Option<String>> {
    match file_path.try_exists() {
        Ok(link_is_ok) => {
            if !link_is_ok {
                return Err(anyhow!(
                    "Failed to read path {:?}. Found broken sym link.",
                    file_path
                ));
            }

            let mut file_content = File::open(&file_path)
                .context(format!("Failed to open config file {:#?}", file_path))?;

            let mut s = String::new();
            file_content.read_to_string(&mut s)?;
            Ok(Some(s.trim().to_string()))
        }
        Err(_) => {
            log::debug!("No user file found at {:?}.", file_path);
            Ok(None)
        }
    }
}
