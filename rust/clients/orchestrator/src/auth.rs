/*
This client is associated with the:
    - AUTH account
    - (the orchestrator's) auth user

This client is responsible for:
    - initalizing connection and handling interface with db
    - registering the `handle_auth_callout` and  `handle_auth_validation` fns as core nats service group endpoints:
        - NB: These endpoints will consider authentiction successful if:
            - user signature is valid
            - hoster pubkey is valid
            - hoster email is valid
            - succesfully paired hoster and host in mongodb
            - succesfully added user to resolver on hub (orchestrator side)
            - succesfully created signed jwt for user
            - succesfully added user jwt file to user collection in mongodb (with ttl)
    - keeping service running until explicitly cancelled out
*/

use anyhow::{anyhow, Context, Result};
use async_nats::service::ServiceExt;
use async_nats::Client;
use authentication::{
    types::AuthErrorPayload, AuthServiceApi, AUTH_CALLOUT_SUBJECT, AUTH_SRV_DESC, AUTH_SRV_NAME,
    AUTH_SRV_SUBJ, AUTH_SRV_VERSION, VALIDATE_AUTH_SUBJECT,
};
use futures::StreamExt;
use mongodb::Client as MongoDBClient;
use nats_utils::{
    jetstream_client::{get_nats_creds_by_nsc, get_nats_url},
    types::CreateResponse,
};
use nkeys::KeyPair;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use std::{sync::Arc, time::Duration};

pub const ORCHESTRATOR_AUTH_CLIENT_NAME: &str = "Orchestrator Auth Manager";
pub const ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX.orchestrator";

pub async fn run(db_client: MongoDBClient) -> Result<Client, async_nats::Error> {
    let admin_account_creds_path =
        PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "AUTH", "auth"))?;

    // Root Keypair associated with AUTH account
    let root_account_key_path = std::env::var("ORCHESTRATOR_ROOT_AUTH_NKEY_PATH")
        .context("Cannot read ORCHESTRATOR_ROOT_AUTH_NKEY_PATH from env var")?;
    let root_account_keypair = Arc::new(
        try_read_keypair_from_file(PathBuf::from_str(&root_account_key_path.clone())?)?
            .ok_or_else(|| {
                anyhow!(
                    "Root AUTH Account keypair not found at path {:?}",
                    root_account_key_path
                )
            })?,
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
            log::debug!("Will time out waiting for NATS after {nats_connect_timeout_secs:?}...");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
         } => {
            return Err(format!("Timed out waiting for NATS on {nats_url}").into());
        }
    };

    // ==================== Setup API & Register Endpoints ====================
    // Generate the Auth API with access to db
    let auth_api = AuthServiceApi::new(&db_client).await?;
    let auth_api_clone = auth_api.clone();

    // Register Auth Service for Orchestrator and spawn listener for processing
    let auth_service = orchestrator_auth_client
        .service_builder()
        .description(AUTH_SRV_DESC)
        .start(AUTH_SRV_NAME, AUTH_SRV_VERSION)
        .await?;

    // Auth Callout Service
    let sys_user_group = auth_service.group("$SYS").group("REQ").group("USER");
    let mut auth_callout = sys_user_group.endpoint(AUTH_CALLOUT_SUBJECT).await?;
    let auth_service_info = auth_service.info().await;
    let orchestrator_auth_client_clone = orchestrator_auth_client.clone();

    tokio::spawn(async move {
        while let Some(request) = auth_callout.next().await {
            let signing_account_kp = Arc::clone(&signing_account_keypair.clone());
            let signing_account_pk = signing_account_pubkey.clone();
            let root_account_kp = Arc::clone(&root_account_keypair.clone());
            let root_account_pk = root_account_pubkey.clone();

            let maybe_reply = request.message.reply.clone();
            match auth_api_clone
                .handle_auth_callout(
                    Arc::new(request.message),
                    signing_account_kp,
                    signing_account_pk,
                    root_account_kp,
                    root_account_pk,
                )
                .await
            {
                Ok(r) => {
                    let res_bytes = r.get_response();
                    if let Some(reply_subject) = maybe_reply {
                        let _ = orchestrator_auth_client_clone
                            .publish(reply_subject, res_bytes)
                            .await
                            .map_err(|e| {
                                log::error!(
                                    "{}Failed to send success response. Res={:?} Err={:?}",
                                    "NATS-SERVICE-LOG::AUTH::",
                                    r,
                                    e
                                );
                            });
                    }
                }
                Err(e) => {
                    let mut err_payload = AuthErrorPayload {
                        service_info: auth_service_info.clone(),
                        group: "$SYS.REQ.USER".to_string(),
                        endpoint: AUTH_CALLOUT_SUBJECT.to_string(),
                        error: format!("{}", e),
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

                    let _ = orchestrator_auth_client_clone
                        .publish(
                            format!("{}.ERROR", ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX),
                            err_response.into(),
                        )
                        .await
                        .map_err(|e| {
                            err_payload.error = e.to_string();
                            log::error!(
                                "{}Failed to send error response. Err={:?}",
                                "NATS-SERVICE-LOG::AUTH::",
                                err_payload
                            );
                        });
                }
            }
        }
    });

    // Auth Validation Service
    let v1_auth_group = auth_service.group(AUTH_SRV_SUBJ); // .group("V1")
    let mut auth_validation = v1_auth_group.endpoint(VALIDATE_AUTH_SUBJECT).await?;
    let orchestrator_auth_client_clone = orchestrator_auth_client.clone();

    tokio::spawn(async move {
        while let Some(request) = auth_validation.next().await {
            let maybe_reply = request.message.reply.clone();
            match auth_api
                .handle_auth_validation(Arc::new(request.message))
                .await
            {
                Ok(r) => {
                    let res_bytes = r.get_response();
                    if let Some(reply_subject) = maybe_reply {
                        let _ = orchestrator_auth_client_clone
                            .publish(reply_subject, res_bytes)
                            .await
                            .map_err(|e| {
                                log::error!(
                                    "{}Failed to send success response. Res={:?} Err={:?}",
                                    "NATS-SERVICE-LOG::AUTH::",
                                    r,
                                    e
                                );
                            });
                    }
                }
                Err(e) => {
                    let auth_service_info = auth_service.info().await;
                    let mut err_payload = AuthErrorPayload {
                        service_info: auth_service_info,
                        group: AUTH_SRV_SUBJ.to_string(),
                        endpoint: VALIDATE_AUTH_SUBJECT.to_string(),
                        error: format!("{}", e),
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
                    let _ = orchestrator_auth_client_clone
                        .publish("AUTH.ERROR", err_response.into())
                        .await
                        .map_err(|e| {
                            err_payload.error = e.to_string();
                            log::error!(
                                "{}Failed to send error response. Err={:?}",
                                "NATS-SERVICE-LOG::AUTH::",
                                err_payload
                            );
                        });
                }
            }
        }
    });

    log::debug!("Orchestrator Auth Service is running. Waiting for requests...");

    Ok(orchestrator_auth_client)
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
