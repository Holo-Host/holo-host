/*
Service Name: AUTH
Subject: "AUTH.>"
Provisioning Account: AUTH Account (ie: This service is exclusively permissioned to the AUTH account.)
Users: orchestrator auth user & auth guard user
Endpoints & Managed Subjects:
    - handle_auth_callout: $SYS.REQ.USER.AUTH
    - handle_auth_validation: AUTH.validate
*/

pub mod types;
pub mod utils;
use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::HeaderValue;
use async_nats::{AuthError, Message};
use bson::{self, doc, to_document};
use core::option::Option::None;
use data_encoding::BASE64URL_NOPAD;
use db_utils::{
    mongodb::{
        api::MongoDbAPI,
        collection::MongoCollection,
        traits::{IntoIndexes, MutMetadata},
    },
    schemas::{self, host::Host, hoster::Hoster, user::User},
};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nats_utils::types::ServiceError;
use nkeys::KeyPair;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use types::{AuthApiResult, DbValidationData};

pub const AUTH_SRV_NAME: &str = "AUTH_SERVICE";
pub const AUTH_SRV_SUBJ: &str = "AUTH";
pub const AUTH_SRV_VERSION: &str = "0.0.1";
pub const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the Host and the Orchestrator.";

// Service Endpoint Names:
// NB: Do not change this subject name unless NATS.io has changed the naming of their auth permissions subject.
// NB: `AUTH_CALLOUT_SUBJECT` attached to the global subject `$SYS.REQ.USER`
pub const AUTH_CALLOUT_SUBJECT: &str = "AUTH";
pub const VALIDATE_AUTH_SUBJECT: &str = "validate";

#[derive(Clone, Debug)]
pub struct AuthServiceApi {
    pub user_collection: MongoCollection<User>,
    pub hoster_collection: MongoCollection<Hoster>,
    pub host_collection: MongoCollection<Host>,
}

impl AuthServiceApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            user_collection: Self::init_collection(client, schemas::user::USER_COLLECTION_NAME)
                .await?,
            hoster_collection: Self::init_collection(
                client,
                schemas::hoster::HOSTER_COLLECTION_NAME,
            )
            .await?,
            host_collection: Self::init_collection(client, schemas::host::HOST_COLLECTION_NAME)
                .await?,
        })
    }

    pub async fn handle_auth_callout(
        &self,
        msg: Arc<Message>,
        auth_signing_account_keypair: Arc<KeyPair>,
        auth_signing_account_pubkey: String,
        auth_root_account_keypair: Arc<KeyPair>,
        auth_root_account_pubkey: String,
    ) -> Result<AuthApiResult, ServiceError> {
        log::info!("Incoming message for '$SYS.REQ.USER.AUTH' : {:#?}", msg);

        // 1. Verify expected data was received
        let auth_request_token = String::from_utf8_lossy(&msg.payload).to_string();
        let auth_request_claim = utils::decode_jwt::<types::NatsAuthorizationRequestClaim>(
            &auth_request_token,
            &auth_signing_account_pubkey,
        )
        .map_err(|e| ServiceError::auth(AuthError::new(e), None))?;

        let auth_request_user_claim = utils::decode_jwt::<types::UserClaim>(
            &auth_request_claim.auth_request.connect_opts.user_jwt,
            &auth_signing_account_pubkey,
        )
        .map_err(|e| ServiceError::auth(AuthError::new(e), None))?;

        if auth_request_user_claim.generic_claim_data.issuer != auth_signing_account_pubkey {
            let e = "Error: Failed to validate issuer for auth user.";
            log::error!("{} Subject='{}'.", e, msg.subject);
            return Err(ServiceError::auth(AuthError::new(e), None));
        };

        // 2. Validate Host signature, returning validation error if not successful
        let user_data = utils::base64_to_data::<types::AuthGuardToken>(
            &auth_request_claim.auth_request.connect_opts.user_auth_token,
        )
        .map_err(|e| ServiceError::auth(AuthError::new(e), None))?;
        let host_pubkey = user_data.host_pubkey.as_ref();
        let host_signature = user_data.get_host_signature();
        let decoded_sig = BASE64URL_NOPAD
            .decode(&host_signature)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;
        let user_verifying_keypair = KeyPair::from_public_key(host_pubkey)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;
        let payload_no_sig = &(user_data.clone().without_signature());
        let raw_payload = serde_json::to_vec(payload_no_sig)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        if let Err(e) = user_verifying_keypair.verify(raw_payload.as_ref(), &decoded_sig) {
            log::error!(
                "Error: Failed to validate Signature. Subject='{}'. Err={}",
                msg.subject,
                e
            );
            return Err(ServiceError::auth(AuthError::new(e), None));
        };

        // 3. Authenticate the Hoster pubkey and email and assign full permissions if successful
        let is_hoster_valid = self
            .verify_is_valid_in_db(user_data.clone())
            .await
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        // 4. Assign permissions based on whether the hoster was successfully validated
        let pubkey_lowercase = host_pubkey.to_lowercase();
        let device_id_lowercase = user_data.device_id.to_lowercase();
        let permissions = if is_hoster_valid {
            // If successful, assign personalized inbox and auth permissions
            let user_unique_auth_subject = &format!("AUTH.{}.>", pubkey_lowercase);
            let user_unique_inbox = &format!("_AUTH_INBOX.{}.>", pubkey_lowercase);
            let authenticated_user_inventory_subject =
                &format!("INVENTORY.{}.>", device_id_lowercase);

            types::Permissions {
                publish: types::PermissionLimits {
                    allow: Some(vec![
                        "AUTH.validate".to_string(),
                        user_unique_auth_subject.to_string(),
                        user_unique_inbox.to_string(),
                        authenticated_user_inventory_subject.to_string(),
                    ]),
                    deny: None,
                },
                subscribe: types::PermissionLimits {
                    allow: Some(vec![
                        user_unique_auth_subject.to_string(),
                        user_unique_inbox.to_string(),
                        authenticated_user_inventory_subject.to_string(),
                    ]),
                    deny: None,
                },
            }
        } else {
            // Otherwise, exclusively grant publication permissions for the unauthenticated inventory subj
            // ...to allow the host device to still send diganostic reports
            let unauthenticated_user_inventory_subject =
                format!("INVENTORY.unauthenticated.{}.update.>", device_id_lowercase);
            types::Permissions {
                publish: types::PermissionLimits {
                    allow: Some(vec![unauthenticated_user_inventory_subject]),
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
        .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        let claim_str = serde_json::to_string(&auth_response_claim)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;
        let token = utils::encode_jwt(&claim_str, &auth_root_account_keypair)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        Ok(types::AuthApiResult {
            result: types::AuthResult::Callout(token),
            maybe_response_tags: None,
        })
    }

    pub async fn handle_auth_validation(
        &self,
        msg: Arc<Message>,
    ) -> Result<AuthApiResult, ServiceError> {
        log::info!("Incoming message for 'AUTH.validate' : {:#?}", msg);

        // 1. Verify expected data was received
        let signature: &[u8] = match &msg.headers {
            Some(h) => {
                let r = HeaderValue::as_str(h.get("X-Signature").ok_or_else(|| {
                    let err_msg = "Error: Missing X-Signature header. Subject='AUTH.authorize'";
                    log::error!("{err_msg}");
                    ServiceError::request(err_msg, Some(ErrorCode::BAD_REQUEST))
                })?);
                r.as_bytes()
            }
            None => {
                let err_msg = "Error: Missing message headers. Subject='AUTH.authorize'";
                log::error!("{err_msg}");
                return Err(ServiceError::request(err_msg, Some(ErrorCode::BAD_REQUEST)));
            }
        };

        let types::AuthJWTPayload {
            device_id,
            host_pubkey,
            maybe_sys_pubkey,
            ..
        } = Self::convert_msg_to_type::<types::AuthJWTPayload>(msg.clone())?;

        // 2. Validate signature
        let decoded_signature = BASE64URL_NOPAD
            .decode(signature)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;
        let user_verifying_keypair = KeyPair::from_public_key(&host_pubkey)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        if let Err(e) = user_verifying_keypair.verify(msg.payload.as_ref(), &decoded_signature) {
            log::error!(
                "Error: Failed to validate Signature. Subject='{}'. Err={}",
                msg.subject,
                e
            );
            return Err(ServiceError::auth(AuthError::new(format!("{:?}", e)), None));
        };

        // 3. Add User keys to nsc resolver (and automatically create account-signed reference to user key)
        utils::add_user_keys_to_resolver(&device_id, &host_pubkey, &maybe_sys_pubkey)
            .await
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        // 4. Create User JWT files (automatically signed with respective account key)
        let (host_jwt, sys_jwt) = utils::create_user_jwt_files(&host_pubkey, &maybe_sys_pubkey)
            .map_err(|e| ServiceError::internal(e.to_string(), None))?;

        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), host_pubkey.clone());

        // 5. Form the result and return
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
    async fn verify_is_valid_in_db(
        &self,
        user_data: types::AuthGuardToken,
    ) -> Result<bool, ServiceError> {
        if let (Some(hoster_hc_pubkey), Some(hoster_email)) =
            (user_data.hoster_hc_pubkey, user_data.email)
        {
            let pipeline = vec![
                // Step 1: Find the `user` document with a matching `hoster_hc_pubkey``
                doc! {
                    "$match": { "hoster.pubkey": hoster_hc_pubkey.clone() }
                },
                // Step 2: Look-up the associated `user_info`` document by referencing the `user.user_info_id` field
                // NB: The `local_field` references a field  local to the `user` document matched in step 1
                doc! {
                    "$lookup": {
                        "from": "user_info",
                        "localField": "user_info_id",
                        "foreignField": "_id",
                        "as": "user_info"
                    }
                },
                // Extract the matching `user_info` document from resulting array
                doc! { "$unwind": "$user_info" },
                doc! {
                    "$lookup": {
                        "from": "hoster",
                        "localField": "hoster.collection_id",
                        "foreignField": "_id",
                        "as": "hoster_record"
                    }
                },
                // Extract the matching `hoster` document from resulting array
                // NB: `hoster` is aliased to `hoster_record` to avoid namespace collision with the `user`` document field `hoster`
                doc! { "$unwind": "$hoster_record" },
                doc! {
                    "$project": {
                        "_id": 0,
                        "jurisdiction": 1,
                        "hoster.pubkey": 1,
                        "hoster_record": 1,
                        "user_info.email": 1,
                    }
                },
            ];

            // Run the aggregation pipeline
            let result = self
                .user_collection
                .aggregate::<DbValidationData>(pipeline)
                .await
                .unwrap_or(vec![]);

            println!("Aggregate pipeline result: {:#?}", result);

            // If no result is returned or more than 1 item exists, call failed
            if result.is_empty() {
                println!("Failed update pipeline...");
                log::error!("DB Authorization Failed. REASON=Failed to locate user collection associated with the valid hoster and user_info document.");
                return Ok(false);
            } else if result.len() > 1 {
                log::warn!("DB Authorization Warning. REASON=Found more than one record matching hoster email and pubkey when validating user data. Defaulting to first record: {:?}", result[0]);
            }

            let DbValidationData {
                jurisdiction: _,
                user_info,
                hoster,
                hoster_pubkey,
            } = &result[0];

            if user_info.email != hoster_email {
                log::error!("DB Authorization Failed. REASON=Invalid hoster email.");
                return Ok(false);
            }

            if hoster_pubkey.pubkey != hoster_hc_pubkey {
                log::error!("DB Authorization Failed. REASON=Invalid hoster pubkey.");
                return Ok(false);
            }

            // Now that host & hoster are successfully validated...
            // Create a new host document in db and assign the bidirectional references
            let new_host = Host::default();
            let filter = doc! { "device_id": user_data.device_id.clone() };
            let update = doc! {
                "$set": {
                    "metadata.updated_at": bson::DateTime::now(),
                    "assigned_hoster": hoster._id,
                },
                // If the document doesn't exist, also set the device_id (host_device_id)
                "$setOnInsert": {
                    "metadata.is_deleted": false,
                    "metadata.created_at": bson::DateTime::now(),
                    "device_id": user_data.device_id,
                    "avg_uptime": new_host.avg_uptime,
                    "avg_network_speed": new_host.avg_network_speed,
                    "avg_latency": new_host.avg_latency,
                    "assigned_workloads": [],
                    "assigned_hoster": hoster._id,
                }
            };

            // Assign Hoster to Host
            let host: Host = self
                .host_collection
                .inner
                .find_one_and_update(filter, UpdateModifications::Document(update))
                .upsert(true)
                .return_document(mongodb::options::ReturnDocument::After)
                .await?
                .ok_or_else(|| {
                    ServiceError::internal(
                        "Failed to return Host record after calling `find_one_and_update`.",
                        Some("Host Collection".to_string()),
                    )
                })?;

            // Assign Host to Hoster
            let mut updated_hoster = hoster.to_owned();
            updated_hoster.assigned_hosts.push(host._id.unwrap());

            self.hoster_collection.update_one_within(
                doc! {
                    "_id": hoster._id
                },
                UpdateModifications::Document(doc! {
                    "$set": to_document(&updated_hoster).map_err(|e| ServiceError::auth(AuthError::new(e), None))?
                }),
                false
            ).await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn init_collection<T>(
        client: &MongoDBClient,
        collection_name: &str,
    ) -> Result<MongoCollection<T>>
    where
        T: Serialize
            + for<'de> Deserialize<'de>
            + Unpin
            + Send
            + Sync
            + Default
            + Debug
            + IntoIndexes
            + MutMetadata,
    {
        Ok(MongoCollection::<T>::new(client, schemas::DATABASE_NAME, collection_name).await?)
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
            ServiceError::request(err_msg, Some(ErrorCode::BAD_REQUEST))
        })
    }
}
