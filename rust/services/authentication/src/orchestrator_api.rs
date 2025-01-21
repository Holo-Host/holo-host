/*
Endpoints & Managed Subjects:
    - handle_handshake_request: AUTH.start_handshake
    - add_user_pubkey: AUTH.handle_handshake_p2
*/

use super::{AuthServiceApi, types, utils};
use anyhow::Result;
use async_nats::{Message, HeaderValue};
use async_nats::jetstream::ErrorCode;
use nkeys::KeyPair;
use types::{AuthApiResult, AuthResult};
use utils::handle_internal_err;
use core::option::Option::None;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use bson::{self, doc, to_document};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use util_libs::{
    nats_js_client::ServiceError,
    db::{
        mongodb::{IntoIndexes, MongoCollection, MongoDbAPI},
        schemas::{
            self,
            User,
            Hoster,
            Host,
            Role,
            RoleInfo,
        }
    },
};

#[derive(Debug, Clone)]
pub struct OrchestratorAuthApi {
    pub user_collection: MongoCollection<User>,
    pub hoster_collection: MongoCollection<Hoster>,
    pub host_collection: MongoCollection<Host>,
}

impl AuthServiceApi for OrchestratorAuthApi {}

impl OrchestratorAuthApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
            hoster_collection: Self::init_collection(client, schemas::HOSTER_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
        })
    }

    /*******************************  For Orchestrator   *********************************/
    // nb: returns to the `save_hub_files` subject
    pub async fn handle_handshake_request(
        &self,
        msg: Arc<Message>,
        creds_dir_path: &str,
    ) -> Result<AuthApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.start_handshake' : {:?}", msg);

        // 1. Verify expected data was received
        let signature: &[u8] = match &msg.headers {
            Some(h) => {
                HeaderValue::as_ref(h.get("X-Signature").ok_or_else(|| {
                    log::error!(
                        "Error: Missing x-signature header. Subject='AUTH.authorize'"
                    );
                    ServiceError::Request(format!("{:?}", ErrorCode::BAD_REQUEST))
                })?)
            },
            None => {
                log::error!(
                    "Error: Missing message headers. Subject='AUTH.authorize'"
                );
                return Err(ServiceError::Request(format!("{:?}", ErrorCode::BAD_REQUEST)));
            }
        };

        let types::AuthRequestPayload { host_pubkey, email, hoster_pubkey, nonce: _ } = Self::convert_to_type::<types::AuthRequestPayload>(msg.clone())?;

        // 2. Validate signature
        let user_verifying_keypair = KeyPair::from_public_key(&host_pubkey).map_err(|e| ServiceError::Internal(e.to_string()))?;
        if let Err(e) = user_verifying_keypair.verify(msg.payload.as_ref(), signature) {
            log::error!("Error: Failed to validate Signature. Subject='{}'. Err={}", msg.subject, e);
            return Err(ServiceError::Request(format!("{:?}", ErrorCode::BAD_REQUEST)));
        };

        // 3. Authenticate the Hosting Agent (via email and host id info?)
        match self.user_collection.get_one_from(doc! { "roles.role.Hoster": hoster_pubkey.clone() }).await? {
            Some(u) => {
                // If hoster exists with pubkey, verify email
                if u.email != email {
                    log::error!("Error: Failed to validate user email. Subject='{}'.", msg.subject);
                    return Err(ServiceError::Request(format!("{:?}", ErrorCode::BAD_REQUEST)));
                }

                // ...then find the host collection that contains the provided host pubkey
                match self.host_collection.get_one_from(doc! { "pubkey": host_pubkey.clone() }).await? {
                    Some(h) => {
                        // ...and pair the host with hoster pubkey (if the hoster is not already assiged to host)
                        if h.assigned_hoster != hoster_pubkey {
                            let host_query: bson::Document = doc! { "_id":  h._id.clone() };
                            let updated_host_doc = to_document(& Host{
                                assigned_hoster: hoster_pubkey,
                                ..h
                            }).map_err(|e| ServiceError::Internal(e.to_string()))?;
                            self.host_collection.update_one_within(host_query, UpdateModifications::Document(updated_host_doc)).await?;                
                        }
                    },
                    None => {
                        let err_msg = format!("Error: Failed to locate Host record. Subject='{}'.", msg.subject);
                        return Err(handle_internal_err(&err_msg));
                    }
                }

                // Find the mongo_id ref for the hoster associated with this user
                let RoleInfo { ref_id, role: _ } = u.roles.into_iter().find(|r| matches!(r.role, Role::Hoster(_))).ok_or_else(|| {
                    let err_msg = format!("Error: Failed to locate Hoster record id in User collection. Subject='{}'.", msg.subject);
                    handle_internal_err(&err_msg)
                })?;
                
                // Finally, find the hoster collection
                match self.hoster_collection.get_one_from(doc! { "_id":  ref_id.clone() }).await? {
                    Some(hr) => {
                        // ...and pair the hoster with host (if the host is not already assiged to the hoster)
                        let mut updated_assigned_hosts = hr.assigned_hosts;
                        if !updated_assigned_hosts.contains(&host_pubkey) {
                        let hoster_query: bson::Document = doc! { "_id":  hr._id.clone() };
                        updated_assigned_hosts.push(host_pubkey.clone());
                        let updated_hoster_doc = to_document(& Hoster {
                            assigned_hosts: updated_assigned_hosts,
                            ..hr
                        }).map_err(|e| ServiceError::Internal(e.to_string()))?;
                        self.host_collection.update_one_within(hoster_query, UpdateModifications::Document(updated_hoster_doc)).await?;                
                        }
                    },
                    None => {
                        let err_msg = format!("Error: Failed to locate Hoster record. Subject='{}'.", msg.subject);
                        return Err(handle_internal_err(&err_msg));
                    }
                }
            },
            None => {
                let err_msg = format!("Error: Failed to find User Collection with Hoster pubkey. Subject='{}'.", msg.subject);
                return Err(handle_internal_err(&err_msg));
            }
        };

        // 4. Read operator and sys account jwts and prepare them to be sent as a payload in the publication callback
        let operator_path = utils::get_file_path_buf(&format!("{}/operator.creds", creds_dir_path));
        let hub_operator_creds: Vec<u8> = std::fs::read(operator_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let sys_path = utils::get_file_path_buf(&format!("{}/sys.creds", creds_dir_path));
        let hub_sys_creds: Vec<u8> = std::fs::read(sys_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), host_pubkey.clone());

        Ok(AuthApiResult {
            status: types::AuthStatus { 
                host_pubkey: host_pubkey.clone(),
                status: types::AuthState::Requested
            },
            result: AuthResult {
                data: types::AuthResultType::Multiple(vec![hub_operator_creds, hub_sys_creds])
            },
            maybe_response_tags: Some(tag_map) // used to inject as tag in response subject
        })
    }

    pub async fn add_user_pubkey(&self, msg: Arc<Message>) -> Result<AuthApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.handle_handshake_p2' : {:?}", msg);

        // 1. Verify expected payload was received
        let host_pubkey = Self::convert_to_type::<String>(msg.clone())?;

        // 2. Add User keys to Orchestrator nsc resolver
        Command::new("nsc")
            .arg("...")
            .output()
            .expect("Failed to add user with provided keys");
        
        // 3. Create and sign User JWT
        let account_signing_key = utils::get_account_signing_key();
        utils::generate_user_jwt(&host_pubkey, &account_signing_key);

        // 4. Prepare User JWT to be sent as a payload in the publication callback
        let user_jwt_path = utils::get_file_path_buf("user_jwt_path");
        let user_jwt: Vec<u8> = std::fs::read(user_jwt_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        // 5. Respond to endpoint request
        Ok(AuthApiResult {
            status: types::AuthStatus { 
                host_pubkey,
                status: types::AuthState::ValidatedAgent
            },
            result: AuthResult {
                data: types::AuthResultType::Single(user_jwt)
            },
            maybe_response_tags: None
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
}
