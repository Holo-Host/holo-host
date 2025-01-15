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
use anyhow::Result;
use async_nats::Message;
use mongodb::Client as MongoDBClient; // options::UpdateModifications, 
use std::process::Command;
use std::sync::Arc;
use std::future::Future;
use serde::{Deserialize, Serialize};
use util_libs::{db::{mongodb::{IntoIndexes, MongoCollection}, schemas}, nats_js_client};

pub const AUTH_SRV_NAME: &str = "AUTH";
pub const AUTH_SRV_SUBJ: &str = "AUTH";
pub const AUTH_SRV_VERSION: &str = "0.0.1";
pub const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the HPOS and the Orchestrator.";

#[derive(Debug, Clone)]
pub struct AuthApi {
    pub user_collection: MongoCollection<schemas::User>,
    pub hoster_collection: MongoCollection<schemas::Hoster>,
    pub host_collection: MongoCollection<schemas::Host>,
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
    ) -> nats_js_client::AsyncEndpointHandler<types::ApiResult>
    where
        F: Fn(AuthApi, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<types::ApiResult, anyhow::Error>> + Send + 'static,
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> nats_js_client::JsServiceResponse<types::ApiResult> {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }
    
    /*******************************  For Orchestrator   *********************************/
    pub async fn receive_handshake_request(
        &self,
        msg: Arc<Message>,
    ) -> Result<types::ApiResult, anyhow::Error> {
        // 1. Verify expected data was received
        if msg.headers.is_none() {
            log::error!(
                "Error: Missing headers. Consumer=authorize_ext_client, Subject='/AUTH/authorize'"
            );
            // anyhow!(ErrorCode::BAD_REQUEST)
        }

        // let signature = msg_clone.headers.unwrap().get("Signature").unwrap_or(&HeaderValue::new());

        // match  serde_json::from_str::<types::AuthHeaders>(signature.as_str()) {
        //     Ok(r) => {}
        //     Err(e) => {
        //         log::error!("Error: Failed to deserialize headers. Consumer=authorize_ext_client, Subject='/AUTH/authorize'")
        //         // anyhow!(ErrorCode::BAD_REQUEST)
        //     }
        // }

        // match serde_json::from_slice::<types::AuthPayload>(msg.payload.as_ref()) {
        //     Ok(r) => {}
        //     Err(e) => {
        //         log::error!("Error: Failed to deserialize payload. Consumer=authorize_ext_client, Subject='/AUTH/authorize'")
        //         // anyhow!(ErrorCode::BAD_REQUEST)
        //     }
        // }

        // 2. Authenticate the HPOS client(?via email and host id info?)

        // 3. Publish operator and sys account jwts for orchestrator
        // let hub_operator_account = chunk_and_publish().await; // returns to the `save_hub_files` subject
        // let hub_sys_account = chunk_and_publish().await; // returns to the `save_hub_files` subject

        Ok(types::ApiResult {
            status: types::AuthStatus { 
                host_id: "host_id_placeholder".to_string(),
                status: types::AuthState::Requested
            },
            result: serde_json::to_vec(&"OK")?,
            maybe_response_tags: None
        })
    }

    /*******************************   For Host Agent   *********************************/
     pub async fn save_hub_jwts(&self, _msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        // receive_and_write_file();

        // Respond to endpoint request
        // let response = b"Hello, NATS!".to_vec();
        // Ok(response)

        todo!();
    }

    /*******************************  For Orchestrator   *********************************/
    pub async fn add_user_pubkey(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

        // Add user with Keys and create jwt
        Command::new("nsc")
            .arg("...")
            .output()
            .expect("Failed to add user with provided keys");

        // Output jwt
        let _user_jwt_path = Command::new("nsc")
            .arg("...")
            // .arg(format!("> {}", output_dir))
            .output()
            .expect("Failed to output user jwt to file")
            .stdout;

        // 2. Respond to endpoint request
        // let resposne = user_jwt_path;
        Ok(types::ApiResult {
            status: types::AuthStatus { 
                host_id: "host_id_placeholder".to_string(),
                status: types::AuthState::ValidatedAgent
            },
            result: b"user_jwt_path_placeholder".to_vec(),
            maybe_response_tags: None
        })
    }

    /*******************************   For Host Agent   *********************************/
     pub async fn save_user_jwt(
        &self,
        msg: Arc<Message>,
        _output_dir: &str,
    ) -> Result<types::ApiResult, anyhow::Error> {
        log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

        // utils::receive_and_write_file(msg, output_dir, file_name).await?;

        // 2. Respond to endpoint request
        Ok(types::ApiResult {
            status: types::AuthStatus { 
                host_id: "host_id_placeholder".to_string(),
                status: types::AuthState::Authenticated
            },
            result: b"Hello, NATS!".to_vec(),
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

}

// In orchestrator
// pub async fn send_hub_jwts(
//     &self,
//     msg: Arc<Message>,
// ) -> Result<Vec<u8>, anyhow::Error> {
//     log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

//     utils::chunk_file_and_publish(msg, output_dir, file_name).await?;

//     // 2. Respond to endpoint request
//     let response = b"Hello, NATS!".to_vec();
//     Ok(response)
// }

// In hpos
// pub async fn send_user_pubkey(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
//     // 1. validate nk key...
//     // let auth_endpoint_subject =
//     // format!("AUTH.{}.file.transfer.JWT-operator", "host_id_placeholder"); // endpoint_subject

//     // 2. Update the hub nsc with user pubkey

//     // 3. create signed jwt

//     // 4. `Ack last request and publish the new jwt to for hpos

//     // 5. Respond to endpoint request
//     // let response = b"Hello, NATS!".to_vec();
//     // Ok(response)

//     todo!()
// }

// In orchestrator
// pub async fn send_user_file(
//     &self,
//     msg: Arc<Message>,
// ) -> Result<Vec<u8>, anyhow::Error> {
//     log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

//     utils::chunk_file_and_publish(msg, output_dir, file_name).await?;

//     // 2. Respond to endpoint request
//     let response = b"Hello, NATS!".to_vec();
//     Ok(response)
// }
