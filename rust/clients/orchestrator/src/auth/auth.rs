/*
This service is associated with the:
    - AUTH account
    - (the orchestrator's) auth user

This service is responsible for:
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

use crate::auth::utils;
use crate::types::nats_clients::auth::ORCHESTRATOR_AUTH_CLIENT_INBOX_PREFIX;

use anyhow::{anyhow, Context, Result};
use async_nats::service::ServiceExt;
use async_nats::Client;
use authentication::{
    types::AuthErrorPayload, AuthServiceApi, AUTH_CALLOUT_SUBJECT, AUTH_SRV_DESC, AUTH_SRV_NAME,
    AUTH_SRV_SUBJ, AUTH_SRV_VERSION, VALIDATE_AUTH_SUBJECT,
};
use futures::StreamExt;
use mongodb::Client as MongoDBClient;
use nats_utils::types::GetResponse;

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::vec;

pub async fn run(
    orchestrator_auth_client: Client,
    db_client: MongoDBClient,
) -> Result<Client, async_nats::Error> {
    // Root Keypair associated with AUTH account
    let root_account_key_path = std::env::var("ORCHESTRATOR_ROOT_AUTH_NKEY_PATH")
        .context("Cannot read ORCHESTRATOR_ROOT_AUTH_NKEY_PATH from env var")?;
    let root_account_keypair = Arc::new(
        utils::try_read_keypair_from_file(PathBuf::from_str(&root_account_key_path.clone())?)?
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
        utils::try_read_keypair_from_file(PathBuf::from_str(&signing_account_key_path.clone())?)?
            .ok_or_else(|| {
            anyhow!(
                "Signing AUTH Account keypair not found at path {:?}",
                signing_account_key_path
            )
        })?,
    );
    let signing_account_pubkey = signing_account_keypair.public_key().clone();

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

    // ==================== Close and Clean Client ====================
    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    log::debug!("Closing orchestrator auth service...");

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    orchestrator_auth_client.drain().await?;
    log::debug!("Closed orchestrator auth service");

    Ok(orchestrator_auth_client)
}
