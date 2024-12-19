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

use anyhow::Result;
use async_nats::Message;
use mongodb::Client as MongoDBClient;
use std::process::Command;
use std::sync::Arc;
use util_libs::db::{mongodb::MongoCollection, schemas};

pub const AUTH_SRV_OWNER_NAME: &str = "AUTH_OWNER";
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
        // Create a typed collection for User
        let user_api: MongoCollection<schemas::User> = MongoCollection::<schemas::User>::new(
            client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await?;

        // Create a typed collection for Hoster
        let hoster_api = MongoCollection::<schemas::Hoster>::new(
            &client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await?;

        // Create a typed collection for Host
        let host_api = MongoCollection::<schemas::Host>::new(
            &client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await?;

        Ok(Self {
            user_collection: user_api,
            hoster_collection: hoster_api,
            host_collection: host_api,
        })
    }

    // For orchestrator
    pub async fn receive_handshake_request(
        &self,
        msg: Arc<Message>,
    ) -> Result<Vec<u8>, anyhow::Error> {
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

        let response = serde_json::to_vec(&"OK")?;
        Ok(response)
    }

    // For hpos
    pub async fn save_hub_jwts(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        // receive_and_write_file();

        // Respond to endpoint request
        // let response = b"Hello, NATS!".to_vec();
        // Ok(response)

        todo!();
    }

    // For orchestrator
    pub async fn add_user_pubkey(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

        // Add user with Keys and create jwt
        Command::new("nsc")
            .arg("...")
            .output()
            .expect("Failed to add user with provided keys")
            .stdout;

        // Output jwt
        let user_jwt_path = Command::new("nsc")
            .arg("...")
            // .arg(format!("> {}", output_dir))
            .output()
            .expect("Failed to output user jwt to file")
            .stdout;

        // 2. Respond to endpoint request
        // let resposne = user_jwt_path;
        let response = b"Hello, NATS!".to_vec();
        Ok(response)
    }

    // For hpos
    pub async fn save_user_jwt(
        &self,
        msg: Arc<Message>,
        output_dir: &str,
    ) -> Result<Vec<u8>, anyhow::Error> {
        log::warn!("INCOMING Message for 'AUTH.add' : {:?}", msg);

        // utils::receive_and_write_file(msg, output_dir, file_name).await?;

        // 2. Respond to endpoint request
        let response = b"Hello, NATS!".to_vec();
        Ok(response)
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
