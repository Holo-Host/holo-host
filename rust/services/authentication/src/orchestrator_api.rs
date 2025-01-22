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

        let types::AuthRequestPayload { host_pubkey, email, hoster_pubkey, nonce: _ } = Self::convert_msg_to_type::<types::AuthRequestPayload>(msg.clone())?;

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
        let hub_operator_jwt: Vec<u8> = std::fs::read(operator_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let sys_account_path = utils::get_file_path_buf(&format!("{}/account_sys.creds", creds_dir_path));
        let hub_sys_account_jwt: Vec<u8> = std::fs::read(sys_account_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let workload_account_path = utils::get_file_path_buf(&format!("{}/account_workload.creds", creds_dir_path));
        let hub_workload_account_jwt: Vec<u8> = std::fs::read(workload_account_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), host_pubkey.clone());

        let mut result_hash_map: HashMap<String, Vec<u8>> = HashMap::new();
        result_hash_map.insert("operator_jwt".to_string(), hub_operator_jwt);
        result_hash_map.insert("sys_account_jwt".to_string(), hub_sys_account_jwt);
        result_hash_map.insert("workload_account_jwt".to_string(), hub_workload_account_jwt);

        Ok(AuthApiResult {
            result: AuthResult {
                status: types::AuthStatus { 
                    host_pubkey: host_pubkey.clone(),
                    status: types::AuthState::Requested
                },
                data: types::AuthResultData { inner: result_hash_map }
            },
            maybe_response_tags: Some(tag_map) // used to inject as tag in response subject
        })
    }

    pub async fn add_user_nkey(&self,
        msg: Arc<Message>,
        creds_dir_path: &str,
    ) -> Result<AuthApiResult, ServiceError> {
        let msg_subject = &msg.subject.clone().into_string(); // AUTH.handle_handshake_p2
        log::trace!("Incoming message for '{}'", msg_subject);

        // 1. Verify expected payload was received
        let message_payload = Self::convert_msg_to_type::<AuthResult>(msg.clone())?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let host_user_nkey_bytes = message_payload.data.inner.get("host_user_nkey").ok_or_else(|| {
            let err_msg = format!("Error: . Subject='{}'.", msg_subject);
            handle_internal_err(&err_msg)
        })?;
        let host_user_nkey = Self::convert_to_type::<String>(host_user_nkey_bytes.to_owned(), msg_subject)?;

        let host_pubkey = &message_payload.status.host_pubkey;

        // 2. Add User keys to nsc resolver (and automatically create account-signed refernce to user key)
        Command::new("nsc")
            .arg(format!("add user -a SYS -n user_sys_host_{} -k {}", host_pubkey, host_user_nkey))
            .output()
            .expect("Failed to add host sys user with provided keys");

        Command::new("nsc")
            .arg(format!("add user -a WORKLOAD -n user_host_{} -k {}", host_pubkey, host_user_nkey))
            .output()
            .expect("Failed to add host user with provided keys");

        // ..and push auth updates to hub server
        Command::new("nsc")
            .arg("push -A")
            .output()
            .expect("Failed to update resolver config file");    

        // 3. Create User JWT files (automatically signed with respective account key)
        let host_sys_user_file_name = format!("{}/user_sys_host_{}.jwt", creds_dir_path, host_pubkey);
        Command::new("nsc") 
            .arg(format!("describe user -a SYS -n user_sys_host_{} --raw --output-file {}", host_pubkey, host_sys_user_file_name))
            .output()
            .expect("Failed to generate host sys user jwt file");

        let host_user_file_name = format!("{}/user_host_{}.jwt", creds_dir_path, host_pubkey);
        Command::new("nsc") 
            .arg(format!("describe user -a WORKLOAD -n user_host_{} --raw --output-file {}", host_pubkey, host_user_file_name))
            .output()
            .expect("Failed to generate host user jwt file");        
    
        // let account_signing_key = utils::get_account_signing_key();
        // utils::generate_user_jwt(&user_nkey, &account_signing_key);

        // 4. Prepare User JWT to be sent as a payload in the publication callback
        let host_sys_user_jwt_path = utils::get_file_path_buf(&host_sys_user_file_name);
        let host_sys_user_jwt: Vec<u8> = std::fs::read(host_sys_user_jwt_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        let host_user_jwt_path = utils::get_file_path_buf(&host_user_file_name);
        let host_user_jwt: Vec<u8> = std::fs::read(host_user_jwt_path).map_err(|e| ServiceError::Internal(e.to_string()))?;
        
        let mut result_hash_map: HashMap<String, Vec<u8>> = HashMap::new();
        result_hash_map.insert("host_sys_user_jwt".to_string(), host_sys_user_jwt);
        result_hash_map.insert("host_user_jwt".to_string(), host_user_jwt);

        // 5. Respond to endpoint request
        Ok(AuthApiResult {
            result: AuthResult {
                status: types::AuthStatus { 
                    host_pubkey: message_payload.status.host_pubkey,
                    status: types::AuthState::ValidatedAgent
                },
                data: types::AuthResultData { inner: result_hash_map }
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
