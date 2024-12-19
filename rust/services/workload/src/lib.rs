/*
Service Name: WORKLOAD
Subject: "WORKLOAD.>"
Provisioning Account: ORCHESTRATOR Account
Importing Account: HPOS Account
Endpoints & Managed Subjects:
- `add_workload`: handles the "WORKLOAD.add" subject
- `handle_changed_workload`: handles the "WORKLOAD.handle_change" subject // the stream changed output by the mongo<>nats connector (stream eg: DB_COLL_CHANGE_WORKLOAD).
- TODO: `start_workload`: handles the "WORKLOAD.start.{{hpos_id}}" subject
- TODO: `remove_workload`: handles the "WORKLOAD.remove.{{hpos_id}}" subject

*/

use anyhow::Result;
use async_nats::Message;
use mongodb::Client as MongoDBClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use util_libs::db::{mongodb::MongoCollection, schemas};

pub const WORKLOAD_SRV_OWNER_NAME: &str = "WORKLOAD_OWNER";
pub const WORKLOAD_SRV_NAME: &str = "WORKLOAD";
pub const WORKLOAD_SRV_SUBJ: &str = "WORKLOAD";
pub const WORKLOAD_SRV_VERSION: &str = "0.0.1";
pub const WORKLOAD_SRV_DESC: &str = "This service handles the flow of Workload requests between the Developer and the Orchestrator, and between the Orchestrator and HPOS.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadState {
    Reported,
    Started,
    Pending,
    Running,
    Failed,
    Unknown(String),
}

#[derive(Debug, Clone)]
pub struct WorkloadApi {
    pub workload_collection: MongoCollection<schemas::Workload>,
    pub host_collection: MongoCollection<schemas::Host>,
    pub user_collection: MongoCollection<schemas::User>,
}

impl WorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        // Create a typed collection for Workload
        let workload_api: MongoCollection<schemas::Workload> =
            MongoCollection::<schemas::Workload>::new(
                client,
                schemas::DATABASE_NAME,
                schemas::HOST_COLLECTION_NAME,
            )
            .await?;

        // Create a typed collection for User
        let user_api = MongoCollection::<schemas::User>::new(
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
            workload_collection: workload_api,
            host_collection: host_api,
            user_collection: user_api,
        })
    }

    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        let payload_buf = msg.payload.to_vec();
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("Incoming message to add workload. Workload={:#?}", workload);

        // 1. Add new workload data into mongodb collection
        // let workload_id = self.workload_collection.insert_one_into(workload).await?;
        // log::info!(
        //     "Successfully added new workload into the Workload Collection. MongodDB Workload ID={}",
        //     workload_id
        // );

        // 2. Respond to endpoint request
        let result = WorkloadState::Reported;
        Ok(serde_json::to_vec(&result)?)
    }

    // NB: This is the stream that is automatically published to by the nats-db-connector
    pub async fn handle_db_change(&self, _msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        // 1. Map over workload items in message and grab capacity requirements

        // 2. Call mongodb to get host collection to get host info and filter by capacity availability

        // 3. Randomly choose host/node

        // 4. Respond to endpoint request
        let response = b"Successfully handled updated workload!".to_vec();
        Ok(response)
    }

    pub async fn start_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::warn!("INCOMING Message for 'WORKLOAD.start' : {:?}", msg);

        let payload_buf = msg.payload.to_vec();
        let _workload = serde_json::from_slice::<schemas::Workload>(&payload_buf)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to install workload...
        // eg: nix_install_with(workload)

        // 2. Respond to endpoint request
        let result = WorkloadState::Started;
        Ok(serde_json::to_vec(&result)?)
    }

    pub async fn signal_status_update(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::warn!("INCOMING Message for 'WORKLOAD.remove' : {:?}", msg);

        let payload_buf = msg.payload.to_vec();
        let workload_state = serde_json::from_slice::<WorkloadState>(&payload_buf)?;

        // Send updated reponse:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(serde_json::to_vec(&workload_state)?)
    }

    pub async fn remove_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        let payload_buf = msg.payload.to_vec();
        let _workload_id = serde_json::from_slice::<String>(&payload_buf)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to UNinstall workload...
        // nix_uninstall_with(workload_id)

        // 2. Respond to endpoint request
        let result = WorkloadState::Pending;
        Ok(serde_json::to_vec(&result)?)
    }
}
