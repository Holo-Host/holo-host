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
use anyhow::Result;
use async_nats::Message;
use async_nats::jetstream::ErrorCode;
use std::sync::Arc;
use std::future::Future;
use types::{WORKLOAD_SK_ROLE, AuthApiResult};
use util_libs::nats_js_client::{ServiceError, AsyncEndpointHandler, JsServiceResponse};
use async_nats::HeaderValue;
use nkeys::KeyPair;
use utils::handle_internal_err;
use core::option::Option::None;
use std::collections::HashMap;
use std::process::Command;
use serde::{Deserialize, Serialize};
use bson::{self, doc, to_document};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use util_libs::db::{mongodb::{IntoIndexes, MongoCollection, MongoDbAPI}, 
    schemas::{
        self,
        User,
        Hoster,
        Host,
        Role,
        RoleInfo
    },
};


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
            hoster_collection: Self::init_collection(client, schemas::HOSTER_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
        })
    }

    pub async fn handle_handshake_request(
        &self,
        msg: Arc<Message>,
        creds_dir_path: &str,
    ) -> Result<AuthApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.validate' : {:?}", msg);

        let mut status = types::AuthState::Unauthenticated;

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

        let types::AuthRequestPayload { host_pubkey, email, hoster_pubkey, sys_pubkey, nonce: _ } = Self::convert_msg_to_type::<types::AuthRequestPayload>(msg.clone())?;

        // 2. Validate signature
        let user_verifying_keypair = KeyPair::from_public_key(&host_pubkey).map_err(|e| ServiceError::Internal(e.to_string()))?;
        if let Err(e) = user_verifying_keypair.verify(msg.payload.as_ref(), signature) {
            log::error!("Error: Failed to validate Signature. Subject='{}'. Err={}", msg.subject, e);
            return Err(ServiceError::Request(format!("{:?}", ErrorCode::BAD_REQUEST)));
        };

        // 3. Authenticate the Hosting Agent (via email and host id info?)
        let hoster_pubkey_as_holo_hash = "convert_hoster_pubkey_to_raw_value_and_then_into_holo_hash";
        match self.user_collection.get_one_from(doc! { "roles.role.Hoster": hoster_pubkey_as_holo_hash.clone() }).await? {
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

        // 4. Add User keys to nsc resolver (and automatically create account-signed refernce to user key)
        Command::new("nsc")
            .arg(format!("add user -a SYS -n user_sys_host_{} -k {}", host_pubkey, sys_pubkey))
            .output()
            .expect("Failed to add host sys user with provided keys");

        Command::new("nsc")
            .arg(format!("add user -a WORKLOAD -n user_host_{} -k {} -K {} --tag pubkey:{}", host_pubkey, host_pubkey, WORKLOAD_SK_ROLE, host_pubkey))
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

        let mut tag_map: HashMap<String, String> = HashMap::new();
        tag_map.insert("host_pubkey".to_string(), host_pubkey.clone());

        status = types::AuthState::Authenticated;
        
        Ok(AuthApiResult {
            host_pubkey: host_pubkey.clone(),
            status,
            maybe_response_tags: Some(tag_map)
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

    pub fn call<F, Fut>(
        &self,
        handler: F,
    ) -> AsyncEndpointHandler<AuthApiResult>
    where
        F: Fn(Self, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AuthApiResult, ServiceError>> + Send + 'static,
        Self: Send + Sync
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> JsServiceResponse<AuthApiResult> {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }

    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!("Error: Failed to deserialize payload. Subject='{}' Err={}", msg.subject.clone().into_string(), e);
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{} Code={:?}", err_msg, ErrorCode::BAD_REQUEST))
        })
    }

}
