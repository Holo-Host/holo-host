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

use async_nats::service::ServiceExt;
use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
// use async_nats::Message;
use authentication::{
    self,
    types::{self, AuthErrorPayload},
    AuthServiceApi, AUTH_SRV_DESC, AUTH_SRV_NAME, AUTH_SRV_SUBJ, AUTH_SRV_VERSION,
};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use nkeys::KeyPair;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{sync::Arc, time::Duration};
use std::str::FromStr;
use util_libs::{
    db::mongodb::get_mongodb_url,
    nats_js_client::{get_nats_url, get_nats_creds_by_nsc}
};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Manager";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX_ORCHESTRATOR";

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

    // Root Keypair associated with AUTH account
    let root_account_key_path = std::env::var("ORCHESTRATOR_ROOT_AUTH_NKEY_PATH")
        .context("Cannot read ORCHESTRATOR_ROOT_AUTH_NKEY_PATH from env var")?;

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
    let root_account_pubkey = root_account_keypair.public_key().clone();

    // AUTH Account Signing Keypair associated with the `auth` user
    let signing_account_key_path = std::env::var("ORCHESTRATOR_SIGNING_AUTH_NKEY_PATH")
    .context("Cannot read ORCHESTRATOR_SIGNING_AUTH_NKEY_PATH from env var")?;
    let signing_account_keypair = Arc::new(
        try_read_keypair_from_file(PathBuf::from_str(&signing_account_key_path.clone())?)?
            .ok_or_else(|| {
                anyhow!(
                    "Signing AUTH Account keypair not found at path {:?}",
                    signing_account_key_path
                )
            })?,
    );
    let signing_account_pubkey = signing_account_keypair.public_key().clone();
    println!(
        ">>>>>>>>> signing_account pubkey: {:?}",
        signing_account_pubkey
    );

    // ==================== Setup NATS ====================
    let nats_url = get_nats_url();
    let nats_connect_timeout_secs: u64 = 180;

    let orchestrator_auth_client = tokio::select! {
        client = async {loop {
            let orchestrator_auth_client = async_nats::ConnectOptions::new()
                .name(ORCHESTRATOR_AUTH_CLIENT_NAME.to_string())
                .custom_inbox_prefix(ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX.to_string())
                .ping_interval(Duration::from_secs(10))
                .request_timeout(Some(Duration::from_secs(30)))
                .credentials_file(&admin_account_creds_path).await.map_err(|e| anyhow::anyhow!("Error loading credentials file: {e}"))?
                .connect(nats_url.clone())
                .await
                .map_err(|e| anyhow::anyhow!("Connecting Orchestrator Auth Client to NATS via {nats_url}: {e}"));

                match orchestrator_auth_client {
                    Ok(client) => break Ok::<async_nats::Client, async_nats::Error>(client),
                    Err(e) => {
                        let duration = tokio::time::Duration::from_millis(100);
                        log::warn!("{}, retrying in {duration:?}", e);
                        tokio::time::sleep(duration).await;
                    }
                }
            }} => client?,
        _ = {
            log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
         } => {
            return Err(format!("timed out waiting for NATS on {nats_url}").into());
        }
    };

    // ==================== Setup DB ====================
    // Create a new MongoDB Client and connect it to the cluster
    let mongo_uri = get_mongodb_url();
    let client_options = ClientOptions::parse(mongo_uri).await?;
    let db_client = MongoDBClient::with_options(client_options)?;
    
    // ==================== Setup API & Register Endpoints ====================
    // Generate the Auth API with access to db
    let auth_api = AuthServiceApi::new(&db_client).await?;
    let auth_api_clone  = auth_api.clone();

    // Register Auth Service for Orchestrator and spawn listener for processing
    let auth_service = orchestrator_auth_client
        .service_builder()
        .description(AUTH_SRV_DESC)
        .start(AUTH_SRV_NAME, AUTH_SRV_VERSION)
        .await?;
    
    // Auth Callout Service
    let sys_user_group = auth_service.group("$SYS").group("REQ").group("USER");
    let mut auth_callout = sys_user_group.endpoint("AUTH").await?;
    let auth_service_info = auth_service.info().await;
    let orchestrator_auth_client_clone = orchestrator_auth_client.clone();

    tokio::spawn(async move {
        while let Some(request) = auth_callout.next().await {
                let signing_account_kp = Arc::clone(&signing_account_keypair);
                let signing_account_pk = signing_account_pubkey.clone();
                let root_account_kp = Arc::clone(&root_account_keypair);
                let root_account_pk = root_account_pubkey.clone();

                if let Err(e) = auth_api_clone.handle_auth_callout(
                    Arc::new(request.message),
                    signing_account_kp,
                    signing_account_pk,
                    root_account_kp,
                    root_account_pk,
                )
                .await {
                    let mut err_payload = AuthErrorPayload {
                        service_info: auth_service_info.clone(),
                        group: "$SYS.REQ.USER".to_string(),
                        endpoint: "AUTH".to_string(),
                        error: format!("{}",e),
                    };

                    log::error!(
                        "{}Failed to handle the endpoint handler. Err={:?}",
                        "NATS-SERVICE-LOG::AUTH::",
                        err_payload
                    );

                    let err_response = serde_json::to_vec(&err_payload).unwrap_or_else(|e| {
                        err_payload.error = e.to_string();
                        log::error!(
                            "{}Failed to deserialize error response. Err={:?}",
                            "NATS-SERVICE-LOG::AUTH::",
                            err_payload
                        );
                        vec![]
                    });

                    let _ = orchestrator_auth_client_clone.publish("_AUTH_INBOX.ERROR", err_response.into()).await.map_err(|e| {
                        log::error!(
                            "{}Failed to send error response. Err={:?}",
                            "NATS-SERVICE-LOG::AUTH::",
                            err_payload
                        );
                    });
                }
            }
        });

        // Auth Validation Service
        let v1_auth_group = auth_service.group(AUTH_SRV_SUBJ).group("V1");
        let mut auth_validation = v1_auth_group.endpoint(types::AUTHORIZE_SUBJECT).await?;
        let orchestrator_auth_client_clone = orchestrator_auth_client.clone();

        tokio::spawn(async move {
            while let Some(request) = auth_validation.next().await {
                if let Err(e) = auth_api.handle_handshake_request(
                    Arc::new(request.message)
                )
                .await {
                    let auth_service_info = auth_service.info().await;
                    let mut err_payload = AuthErrorPayload {
                        service_info: auth_service_info,
                        group: "AUTH.V1".to_string(),
                        endpoint: types::AUTHORIZE_SUBJECT.to_string(),
                        error: format!("{}",e),
                    };
                    log::warn!(
                        "{}Failed to handle the endpoint handler. Err={:?}",
                        "NATS-SERVICE-LOG::AUTH::",
                        err_payload
                    );
                    let err_response = serde_json::to_vec(&err_payload).unwrap_or_else(|e| {
                        err_payload.error = e.to_string();
                        log::error!(
                            "{}Failed to deserialize error response. Err={:?}",
                            "NATS-SERVICE-LOG::AUTH::",
                            err_payload
                        );
                        vec![]
                    });
                    let _ = orchestrator_auth_client_clone.publish("_AUTH_INBOX.ERROR", err_response.into()).await.map_err(|e| {
                        log::error!(
                            "{}Failed to send error response. Err={:?}",
                            "NATS-SERVICE-LOG::AUTH::",
                            err_payload
                        );
                    });
                }
            }
        });

    println!("Orchestrator Auth Service is running. Waiting for requests...");

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    println!("closing orchestrator auth service...");

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_auth_client.drain().await?;
    println!("closed orchestrator auth service");

    Ok(())
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