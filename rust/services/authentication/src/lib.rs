/*
Service Name: AUTH
Subject: "AUTH.>"
Provisioning Account: AUTH Account
Importing Account: Auth/NoAuth Account

This service should be run on the ORCHESTRATOR side and called from the HPOS side.
The NoAuth/Auth Server will import this service on the hub side and read local jwt files once the agent is validated.
NB: subject pattern = "<SERVICE_NAME>.<Subject>.<DirectObject>.<Verb>.<Details>"
This service handles the the "AUTH.<host_id>.file.transfer.JWT-<hoster_pubkey>.<chunk_id>" subject

Endpoints & Managed Subjects:
    - start_hub_handshake
    - end_hub_handshake
    - save_hub_auth
    - save_user_auth

*/

pub mod types;
pub mod utils;

use anyhow::Result;
use async_nats::{Message, HeaderValue};
use async_nats::jetstream::ErrorCode;
use nkeys::KeyPair;
use types::AuthResult;
use utils::handle_internal_err;
use core::option::Option::None;
use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::future::Future;
use serde::{Deserialize, Serialize};
use bson::{self, doc, to_document};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use util_libs::{
    nats_js_client::{ServiceError, AsyncEndpointHandler, JsServiceResponse},
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

pub const AUTH_SRV_NAME: &str = "AUTH";
pub const AUTH_SRV_SUBJ: &str = "AUTH";
pub const AUTH_SRV_VERSION: &str = "0.0.1";
pub const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the HPOS and the Orchestrator.";

#[derive(Debug,
    Clone)]
pub struct AuthApi {
    pub user_collection: MongoCollection<User>,
    pub hoster_collection: MongoCollection<Hoster>,
    pub host_collection: MongoCollection<Host>,
}

impl AuthApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
            hoster_collection: Self::init_collection(client, schemas::HOSTER_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
        })
    }

    pub fn call<F, Fut>(
        &self,
        handler: F,
    ) -> AsyncEndpointHandler<types::ApiResult>
    where
        F: Fn(AuthApi, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<types::ApiResult, ServiceError>> + Send + 'static,
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> JsServiceResponse<types::ApiResult> {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }
    
    /*******************************  For Orchestrator   *********************************/
    // nb: returns to the `save_hub_files` subject
    pub async fn handle_handshake_request(
        &self,
        msg: Arc<Message>,
        creds_dir_path: &str,
    ) -> Result<types::ApiResult, ServiceError> {
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
                    Some(host_collection) => {
                        // ...and pair the host with hoster pubkey (if the hoster is not already assiged to host)
                        if host_collection.assigned_hoster != hoster_pubkey {
                            let host_query: bson::Document = doc! { "_id":  host_collection._id.clone() };
                            let updated_host_doc = to_document(& Host{
                                assigned_hoster: hoster_pubkey,
                                ..host_collection
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
                    Some(hoster_collection) => {
                        // ...and pair the hoster with host (if the host is not already assiged to the hoster)
                        let mut updated_assigned_hosts = hoster_collection.assigned_hosts;
                        if !updated_assigned_hosts.contains(&host_pubkey) {
                        let hoster_query: bson::Document = doc! { "_id":  hoster_collection._id.clone() };
                        updated_assigned_hosts.push(host_pubkey.clone());
                        let updated_hoster_doc = to_document(& Hoster {
                            assigned_hosts: updated_assigned_hosts,
                            ..hoster_collection
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

        Ok(types::ApiResult {
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

    /*******************************   For Host Agent   *********************************/
     pub async fn save_hub_jwts(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.<host_pubkey>.handle_handshake_p1' : {:?}", msg);

        // receive_and_write_file();

        // Respond to endpoint request
        // let response = b"Hello, NATS!".to_vec();

        // let resolver_path = utils::get_resolver_path();

        // // Generate resolver file and create resolver file
        // Command::new("nsc")
        //     .arg("generate")
        //     .arg("config")
        //     .arg("--nats-resolver")
        //     .arg("sys-account SYS")
        //     .arg("--force")
        //     .arg(format!("--config-file {}", resolver_path))
        //     .output()
        //     .expect("Failed to create resolver config file");

        // // Push auth updates to hub server
        // Command::new("nsc")
        //     .arg("push -A")
        //     .output()
        //     .expect("Failed to create resolver config file");

        // Ok(response)

        todo!();
    }

    /*******************************  For Orchestrator   *********************************/
    pub async fn add_user_pubkey(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
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
        let sys_path = utils::get_file_path_buf("user_jwt_path");
        let user_jwt: Vec<u8> = std::fs::read(sys_path).map_err(|e| ServiceError::Internal(e.to_string()))?;

        // 5. Respond to endpoint request
        Ok(types::ApiResult {
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

    /*******************************   For Host Agent   *********************************/
     pub async fn save_user_jwt(
        &self,
        msg: Arc<Message>,
        _output_dir: &str,
    ) -> Result<types::ApiResult, ServiceError> {
        log::warn!("INCOMING Message for 'AUTH.<host_pubkey>.end_handshake' : {:?}", msg);

        // Generate user jwt file
        // utils::receive_and_write_file(msg, output_dir, file_name).await?;
        
        // Generate user creds file
        // let _user_creds_path = utils::generate_creds_file();

        // 2. Respond to endpoint request
        Ok(types::ApiResult {
            status: types::AuthStatus { 
                host_pubkey: "host_id_placeholder".to_string(),
                status: types::AuthState::Authenticated
            },
            result: AuthResult {
                data: types::AuthResultType::Single(b"Hello, NATS!".to_vec())
            },
            maybe_response_tags: None
        })
    }

    /*******************************  Helper Fns  *********************************/
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

    fn convert_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!("Error: Failed to deserialize payload. Subject='{}' Err={}", msg.subject, e);
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{} Code={:?}", err_msg, ErrorCode::BAD_REQUEST))
        })
    }

}

// In hpos
// pub async fn send_user_pubkey(&self, msg: Arc<Message>) -> Result<Vec<u8>, ServiceError> {
//     // 1. validate nk key...
//     // let auth_endpoint_subject =
//     // format!("AUTH.{}.file.transfer.JWT-operator", "host_id_placeholder"); // endpoint_subject

//     // 2. Update the hub nsc with user pubkey

//     // 3. create signed jwt

//     // 4. `Ack last msg and publish the new jwt to for hpos

//     // 5. Respond to endpoint request
//     // let response = b"Hello, NATS!".to_vec();
//     // Ok(response)

//     todo!()
// }
