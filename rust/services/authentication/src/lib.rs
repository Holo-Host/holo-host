/*
Service Name: AUTH
Subject: "AUTH.>"
Provisioning Account: ADMIN Account (ie: This service is exclusively permissioned to the ADMIN account.)
Users: orchestrator & noauth
Endpoints & Managed Subjects:
    - handle_handshake_request: AUTH.validate
*/

pub mod types;
pub mod utils;
use anyhow::{Context, Result};
use async_nats::jetstream::ErrorCode;
use async_nats::HeaderValue;
use async_nats::{AuthError, Message};
use bson::{self, doc, to_document};
use core::option::Option::None;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nkeys::KeyPair;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::process::Command;
use std::sync::Arc;
use types::{AuthApiResult, WORKLOAD_SK_ROLE};
use util_libs::db::{
    mongodb::{IntoIndexes, MongoCollection, MongoDbAPI},
    schemas::{self, Host, Hoster, Role, RoleInfo, User},
};
use util_libs::nats_js_client::{AsyncEndpointHandler, JsServiceResponse, ServiceError};
use utils::handle_internal_err;

pub const AUTH_SRV_NAME: &str = "AUTH";
pub const AUTH_SRV_SUBJ: &str = "AUTH";
pub const AUTH_SRV_VERSION: &str = "0.0.1";
pub const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the Host and the Orchestrator.";

#[derive(Clone, Debug)]
pub struct AuthServiceApi {
    pub user_collection: MongoCollection<User>,
    pub hoster_collection: MongoCollection<Hoster>,
    pub host_collection: MongoCollection<Host>,
}

impl AuthServiceApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
            hoster_collection: Self::init_collection(client, schemas::HOSTER_COLLECTION_NAME)
                .await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
        })
    }

    pub async fn handle_auth_callout(
        &self,
        msg: Arc<Message>,
        auth_signing_account_keypair: KeyPair,
        auth_signing_account_pubkey: String,
        auth_root_account_keypair: KeyPair,
        auth_root_account_pubkey: String,
    ) -> Result<AuthApiResult, ServiceError> {
        // 1. Verify expected data was received
        let auth_request_token = String::from_utf8_lossy(&msg.payload).to_string();
        println!("auth_request_token  : {:?}", auth_request_token);

        let auth_request_claim =
            utils::decode_jwt::<types::NatsAuthorizationRequestClaim>(&auth_request_token)
                .map_err(|e| ServiceError::Authentication(AuthError::new(e)))?;
        println!(
            "\nauth REQUEST - main claim : {}",
            serde_json::to_string_pretty(&auth_request_claim).unwrap()
        );

        let auth_request_user_claim = utils::decode_jwt::<types::UserClaim>(
            &auth_request_claim.auth_request.connect_opts.user_jwt,
        )
        .map_err(|e| ServiceError::Authentication(AuthError::new(e)))?;
        println!(
            "\nauth REQUEST - user claim : {}",
            serde_json::to_string_pretty(&auth_request_user_claim).unwrap()
        );

        let user_data: types::AuthGuardPayload = utils::base64_to_data::<types::AuthGuardPayload>(
            &auth_request_claim.auth_request.connect_opts.user_auth_token,
        )
        .map_err(|e| ServiceError::Authentication(AuthError::new(e)))?;
        println!("user_data TO VALIDATE  : {:#?}", user_data);

        // 2. Validate Host signature, returning validation error if not successful
        let host_pubkey = user_data.host_pubkey.as_ref();
        let host_signature = user_data.get_host_signature();
        let user_verifying_keypair = KeyPair::from_public_key(host_pubkey)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        let raw_payload = serde_json::to_vec(&user_data.clone().without_signature())
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        if let Err(e) = user_verifying_keypair.verify(raw_payload.as_ref(), &host_signature) {
            log::error!(
                "Error: Failed to validate Signature. Subject='{}'. Err={}",
                msg.subject,
                e
            );
            return Err(ServiceError::Authentication(AuthError::new(e)));
        };

        // 3. If provided, authenticate the Hoster pubkey and email and assign full permissions if successful
        let is_hoster_valid = if user_data.email.is_some() && user_data.hoster_hc_pubkey.is_some() {
            let hoster_hc_pubkey = user_data.hoster_hc_pubkey.unwrap(); // unwrap is safe here as checked above
            let hoster_email = user_data.email.unwrap(); // unwrap is safe here as checked above

            let is_valid: bool = match self
                .user_collection
                .get_one_from(doc! { "roles.role.Hoster": hoster_hc_pubkey.clone() })
                .await?
            {
                Some(u) => {
                    let mut is_valid = true;
                    // If hoster exists with pubkey, verify email
                    if u.email != hoster_email {
                        log::error!(
                            "Error: Failed to validate hoster email. Email='{}'.",
                            hoster_email
                        );
                        is_valid = false;
                    }

                    // ...then find the host collection that contains the provided host pubkey
                    match self
                        .host_collection
                        .get_one_from(doc! { "pubkey": host_pubkey })
                        .await?
                    {
                        Some(host) => {
                            // ...and pair the host with hoster pubkey (if the hoster is not already assiged to host)
                            if host.assigned_hoster != hoster_hc_pubkey {
                                let host_query: bson::Document = doc! { "_id":  host._id.clone() };
                                let updated_host_doc = to_document(&Host {
                                    assigned_hoster: hoster_hc_pubkey,
                                    ..host
                                })
                                .map_err(|e| ServiceError::Internal(e.to_string()))?;

                                self.host_collection
                                    .update_one_within(
                                        host_query,
                                        UpdateModifications::Document(updated_host_doc),
                                    )
                                    .await?;
                            }
                        }
                        None => {
                            log::error!(
                                "Error: Failed to locate Host record. Subject='{}'.",
                                msg.subject
                            );
                            is_valid = false;
                        }
                    }

                    // Find the mongo_id ref for the hoster associated with this user
                    let RoleInfo { ref_id, role: _ } = u.roles.into_iter().find(|r| matches!(r.role, Role::Hoster(_))).ok_or_else(|| {
                        let err_msg = format!("Error: Failed to locate Hoster record id in User collection. Subject='{}'.", msg.subject);
                        handle_internal_err(&err_msg)
                    })?;

                    // Finally, find the hoster collection
                    match self
                        .hoster_collection
                        .get_one_from(doc! { "_id":  ref_id.clone() })
                        .await?
                    {
                        Some(hoster) => {
                            // ...and pair the hoster with host (if the host is not already assiged to the hoster)
                            let mut updated_assigned_hosts = hoster.assigned_hosts;
                            if !updated_assigned_hosts.contains(&host_pubkey.to_string()) {
                                let hoster_query: bson::Document =
                                    doc! { "_id":  hoster._id.clone() };
                                updated_assigned_hosts.push(host_pubkey.to_string());
                                let updated_hoster_doc = to_document(&Hoster {
                                    assigned_hosts: updated_assigned_hosts,
                                    ..hoster
                                })
                                .map_err(|e| ServiceError::Internal(e.to_string()))?;

                                self.host_collection
                                    .update_one_within(
                                        hoster_query,
                                        UpdateModifications::Document(updated_hoster_doc),
                                    )
                                    .await?;
                            }
                        }
                        None => {
                            log::error!(
                                "Error: Failed to locate Hoster record. Subject='{}'.",
                                msg.subject
                            );
                            is_valid = false;
                        }
                    }
                    is_valid
                }
                None => {
                    log::error!(
                        "Error: Failed to find User Collection with Hoster pubkey. Subject='{}'.",
                        msg.subject
                    );
                    false
                }
            };
            is_valid
        } else {
            false
        };

        // 4. Assign permissions based on whether the hoster was successfully validated
        let permissions = if is_hoster_valid {
            // If successful, assign personalized inbox and auth permissions
            let user_unique_auth_subject = &format!("AUTH.{}.>", host_pubkey);
            println!(">>> user_unique_auth_subject : {user_unique_auth_subject}");

            let user_unique_inbox = &format!("_INBOX.{}.>", host_pubkey);
            println!(">>> user_unique_inbox : {user_unique_inbox}");

            let authenticated_user_diagnostics_subject =
                &format!("DIAGNOSTICS.authenticated.{}.>", host_pubkey);
            println!(">>> authenticated_user_diagnostics_subject : {authenticated_user_diagnostics_subject}");

            types::Permissions {
                publish: types::PermissionLimits {
                    allow: Some(vec![
                        "AUTH.validate".to_string(),
                        user_unique_auth_subject.to_string(),
                        user_unique_inbox.to_string(),
                        authenticated_user_diagnostics_subject.to_string(),
                    ]),
                    deny: None,
                },
                subscribe: types::PermissionLimits {
                    allow: Some(vec![
                        user_unique_auth_subject.to_string(),
                        user_unique_inbox.to_string(),
                        authenticated_user_diagnostics_subject.to_string(),
                    ]),
                    deny: None,
                },
            }
        } else {
            // Otherwise, exclusively grant publication permissions for the unauthenticated diagnostics subj
            // ...to allow the host device to still send diganostic reports
            let unauthenticated_user_diagnostics_subject =
                format!("DIAGNOSTICS.unauthenticated.{}.>", host_pubkey);
            types::Permissions {
                publish: types::PermissionLimits {
                    allow: Some(vec![unauthenticated_user_diagnostics_subject]),
                    deny: None,
                },
                subscribe: types::PermissionLimits {
                    allow: None,
                    deny: Some(vec![">".to_string()]),
                },
            }
        };

        let auth_response_claim = utils::generate_auth_response_claim(
            auth_signing_account_keypair,
            auth_signing_account_pubkey,
            auth_root_account_pubkey,
            permissions,
            auth_request_claim,
        )
        .map_err(|e| ServiceError::Internal(e.to_string()))?;

        let claim_str = serde_json::to_string(&auth_response_claim)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        let token = utils::encode_jwt(&claim_str, &auth_root_account_keypair)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        println!("\n\n\n\nencoded_jwt: {:#?}", token);

        // DONE BY JS HANDLER
        // let res = token.into_bytes();
        // if let Some(reply) = msg.reply {
        //     client.publish(reply, res.into()).await?;
        // }

        Ok(types::AuthApiResult {
            result: types::AuthResult::Callout(token),
            maybe_response_tags: None,
        })
    }

    pub async fn handle_handshake_request(
        &self,
        msg: Arc<Message>,
    ) -> Result<AuthApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.validate' : {:?}", msg);

        // 1. Verify expected data was received
        let signature: &[u8] = match &msg.headers {
            Some(h) => HeaderValue::as_ref(h.get("X-Signature").ok_or_else(|| {
                log::error!("Error: Missing x-signature header. Subject='AUTH.authorize'");
                ServiceError::Request(format!("{:?}", ErrorCode::BAD_REQUEST))
            })?),
            None => {
                log::error!("Error: Missing message headers. Subject='AUTH.authorize'");
                return Err(ServiceError::Request(format!(
                    "{:?}",
                    ErrorCode::BAD_REQUEST
                )));
            }
        };

        let types::AuthJWTPayload {
            host_pubkey,
            maybe_sys_pubkey,
            nonce: _,
        } = Self::convert_msg_to_type::<types::AuthJWTPayload>(msg.clone())?;

        // 2. Validate signature
        let user_verifying_keypair = KeyPair::from_public_key(&host_pubkey)
            .map_err(|e| ServiceError::Internal(e.to_string()))?;
        if let Err(e) = user_verifying_keypair.verify(msg.payload.as_ref(), signature) {
            log::error!(
                "Error: Failed to validate Signature. Subject='{}'. Err={}",
                msg.subject,
                e
            );
            return Err(ServiceError::Request(format!(
                "{:?}",
                ErrorCode::BAD_REQUEST
            )));
        };

        // 4. Add User keys to nsc resolver (and automatically create account-signed refernce to user key)
        if let Some(sys_pubkey) = maybe_sys_pubkey {
            Command::new("nsc")
                .arg(format!(
                    "add user -a SYS -n user_sys_host_{} -k {}",
                    host_pubkey, sys_pubkey
                ))
                .output()
                .context("Failed to add host sys user with provided keys")
                .map_err(|e| ServiceError::Internal(e.to_string()))?;

            Command::new("nsc")
                .arg(format!(
                    "add user -a WORKLOAD -n user_host_{} -k {} -K {} --tag pubkey:{}",
                    host_pubkey, host_pubkey, WORKLOAD_SK_ROLE, host_pubkey
                ))
                .output()
                .context("Failed to add host user with provided keys")
                .map_err(|e| ServiceError::Internal(e.to_string()))?;
        }

        // ..and push auth updates to hub server
        Command::new("nsc")
            .arg("push -A")
            .output()
            .context("Failed to update resolver config file")
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        // 3. Create User JWT files (automatically signed with respective account key)
        let sys_jwt_output = Command::new("nsc")
            .arg(format!(
                "describe user -n user_sys_host_{} -a SYS --raw",
                host_pubkey
            ))
            .output()
            .context("Failed to generate host sys user jwt file")
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        let sys_jwt = String::from_utf8(sys_jwt_output.stdout)
            .context("Command returned invalid UTF-8 output")
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        let host_jwt_output = Command::new("nsc")
            .arg(format!(
                "describe user -n user_host_{} -a WORKLOAD --raw",
                host_pubkey
            ))
            .output()
            .context("Failed to generate host user jwt file")
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        let host_jwt = String::from_utf8(host_jwt_output.stdout)
            .context("Command returned invalid UTF-8 output")
            .map_err(|e| ServiceError::Internal(e.to_string()))?;

        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), host_pubkey.clone());

        Ok(AuthApiResult {
            result: types::AuthResult::Authorization(types::AuthJWTResult {
                host_pubkey: host_pubkey.clone(),
                status: types::AuthState::Authorized,
                host_jwt,
                sys_jwt,
            }),
            maybe_response_tags: Some(tag_map),
        })
    }

    // Helper function to initialize mongodb collections
    async fn init_collection<T>(
        client: &MongoDBClient,
        collection_name: &str,
    ) -> Result<MongoCollection<T>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync + Default + IntoIndexes,
    {
        Ok(MongoCollection::<T>::new(client, schemas::DATABASE_NAME, collection_name).await?)
    }

    pub fn call<F, Fut>(&self, handler: F) -> AsyncEndpointHandler<AuthApiResult>
    where
        F: Fn(Self, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AuthApiResult, ServiceError>> + Send + 'static,
        Self: Send + Sync,
    {
        let api = self.to_owned();
        Arc::new(
            move |msg: Arc<Message>| -> JsServiceResponse<AuthApiResult> {
                let api_clone = api.clone();
                Box::pin(handler(api_clone, msg))
            },
        )
    }

    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!(
                "Error: Failed to deserialize payload. Subject='{}' Err={}",
                msg.subject.clone().into_string(),
                e
            );
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{} Code={:?}", err_msg, ErrorCode::BAD_REQUEST))
        })
    }
}
