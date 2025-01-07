/*
Service Name: WORKLOAD
Subject: "WORKLOAD.>"
Provisioning Account: WORKLOAD
Users: orchestrator & hpos
Endpoints & Managed Subjects:
- `add_workload`: handles the "WORKLOAD.add" subject
- `remove_workload`: handles the "WORKLOAD.remove" subject
- `handle_db_change`: handles the "WORKLOAD.handle_change" subject // the stream changed output by the mongo<>nats connector (stream eg: DB_COLL_CHANGE_WORKLOAD).
- TODO: `start_workload`: handles the "WORKLOAD.start.{{hpos_id}}" subject
- TODO: `send_workload_status`: handles the "WORKLOAD.send_status.{{hpos_id}}" subject
- TODO: `uninstall_workload`: handles the "WORKLOAD.uninstall.{{hpos_id}}" subject
*/

use anyhow::Result;
use async_nats::Message;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use std::sync::Arc;
use util_libs::{db::{schemas::{WorkloadState, WorkloadStatus}, mongodb::{MongoCollection, MongoDbPool}, schemas::{self, Host, Workload}}, nats_js_client};
use rand::seq::SliceRandom;
use std::future::Future;
use bson::{self, doc, to_document};

pub const WORKLOAD_SRV_OWNER_NAME: &str = "WORKLOAD_OWNER";
pub const WORKLOAD_SRV_NAME: &str = "WORKLOAD";
pub const WORKLOAD_SRV_SUBJ: &str = "WORKLOAD";
pub const WORKLOAD_SRV_VERSION: &str = "0.0.1";
pub const WORKLOAD_SRV_DESC: &str = "This service handles the flow of Workload requests between the Developer and the Orchestrator, and between the Orchestrator and HPOS.";


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
            client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await?;

        // Create a typed collection for Host
        let host_api = MongoCollection::<schemas::Host>::new(
            client,
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


    pub fn call<F, Fut>(
        &self,
        handler: F,
    ) -> nats_js_client::AsyncEndpointHandler
    where
        F: Fn(WorkloadApi, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<u8>, anyhow::Error>> + Send + 'static,
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> nats_js_client::JsServiceResponse {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }

    // For orchestrator
    // NB: This is the stream that is automatically published to by the nats-db-connector
    pub async fn handle_db_change(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.handle_workload_change'");

        let error_status = WorkloadStatus {
            desired: WorkloadState::Assigned,
            actual: WorkloadState::Reported,
        };
        let success_status = WorkloadStatus {
            desired: WorkloadState::Assigned,
            actual: WorkloadState::Assigned,
        };

        let payload_buf = msg.payload.to_vec();
        // Verify the expected incoming type for the change stream -
        // - is it for a vec or a single type
        // - is it simply the collecion schema type or is there add'l metadata to parse and use?

        // ...IF THE CHANGE IS ADDING A NEW ENTRY INTO THE WORKLOAD COLLECTION:
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);
        // 1a. Map over workload items in message and grab capacity requirements
        // 1b. Check whether the workload is already assigned to a host, and if so, ensure that the host has enough capacity for updated requirements

        // 2. Call mongodb to get host collection to get host info and filter by capacity availability
        let host_filter = doc! {}; // doc! {
        //     "$and": [
        //         { "remaining_capacity.cores": { "$gte": workload.system_specs.capacity.cores } },      
        //         { "remaining_capacity.memory": { "$gte": workload.system_specs.capacity.memory } },
        //         { "remaining_capacity.disk": { "$gte": workload.system_specs.capacity.disk } }
        //     ]
        // };
        let eligible_hosts = self.host_collection.get_many_from(host_filter).await?;
        log::info!(
            "Eligible hosts for new workload. MongodDB Host IDs={:?}",
            eligible_hosts
        );

        // 3. If no host is currently assigned OR the current host has insufficient capacity,
        // randomly choose host/node
        let host = match eligible_hosts.choose(&mut rand::thread_rng()) {
            Some(h) => h,
            None => {
                // todo: Try to get another host up to 5 times, if fails thereafter, return error
                return Ok(serde_json::to_vec(&error_status)?);
            }
        };
        
        // 4. Send the workload request to host
        // publish to `WORKLOAD.add` with workload as payload...

        // 5a. Update the Workload Collection with the assigned Host ID
        let workload_query = doc! { "_id":  workload.clone()._id };
        let updated_workload = to_document(&Workload {
            assigned_hosts: vec![host._id.to_owned()],
            ..workload.clone()
        })?;
        let updated_workload_result = self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(updated_workload)).await?;
        log::info!(
            "Successfully added new workload into the Workload Collection. MongodDB Workload ID={:?}",
            updated_workload_result
        );
        
        // 5b. Update the Host Collection with the assigned Workload ID
        let host_query = doc! { "_id":  host.clone()._id };
        let updated_host =  to_document(&Host {
            assigned_workloads: vec![workload._id],
            ..host.to_owned()
        })?;
        let updated_host_result = self.host_collection.update_one_within(host_query, UpdateModifications::Document(updated_host)).await?;
        log::info!(
            "Successfully added new workload into the Workload Collection. MongodDB Host ID={:?}",
            updated_host_result
        );

        // -- -- -- -- -- -- -- -- --

        // TODO: ...HANDLE WHEN THE CHANGE IS REMOVING AN ENTRY FROM THE WORKLOAD COLLECTION
        
        // TODO: ...HANDLE WHEN THE CHANGE IS UPDATING AN ENTRY FROM THE WORKLOAD COLLECTION
        
        Ok(serde_json::to_vec(&success_status)?)
    }    

    // For orchestrator
    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.add'");

        let _error_status = WorkloadStatus {
            desired: WorkloadState::Reported,
            actual: WorkloadState::Failed,
        };
        let success_status = WorkloadStatus {
            desired: WorkloadState::Reported,
            actual: WorkloadState::Reported,
        };

        let payload_buf = msg.payload.to_vec();
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("Incoming message to add workload. Workload={:#?}", workload);

        // 1. Add new workload data into mongodb collection
        let workload_id = self.workload_collection.insert_one_into(workload).await?;
        log::info!(
            "Successfully added new workload into the Workload Collection. MongodDB Workload ID={}",
            workload_id
        );

        // 2. Respond to endpoint request
        Ok(serde_json::to_vec(&success_status)?)
    }

    // For orchestrator
    pub async fn remove_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.remove'");

        let _error_status = WorkloadStatus {
            desired: WorkloadState::Removed,
            actual: WorkloadState::Running,
        };
        let success_status = WorkloadStatus {
            desired: WorkloadState::Removed,
            actual: WorkloadState::Removed,
        };

        let payload_buf = msg.payload.to_vec();
        let workload_id: String = serde_json::from_slice(&payload_buf)?;
        log::trace!("Incoming message to remove a workload. Workload ID={}", workload_id);

        // 1. Remove workload data into mongodb collection
        let workload_query = doc! { "_id":  workload_id };
        let delete_id = self.workload_collection.delete_one_from(workload_query).await?;
        log::info!(
            "Successfully removed workload from the Workload Collection. MongodDB Workload ID={:?}",
            delete_id
        );

        // 2. Respond to endpoint request
        Ok(serde_json::to_vec(&success_status)?)
    }

    // For hpos
    pub async fn start_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.start' : {:?}", msg);

        let payload_buf = msg.payload.to_vec();
        let _workload = serde_json::from_slice::<schemas::Workload>(&payload_buf)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to install workload...
        // eg: nix_install_with(workload)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            desired: WorkloadState::Running,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(serde_json::to_vec(&status)?)
    }

    // For hpos ? or elsewhere ?
    // TODO: Talk through with Stefan
    pub async fn send_workload_status(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!(
            "Incoming message for 'WORKLOAD.send_workload_status' : {:?}",
            msg
        );

        let payload_buf = msg.payload.to_vec();
        let workload_state = serde_json::from_slice::<WorkloadState>(&payload_buf)?;

        // Send updated status:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(serde_json::to_vec(&workload_state)?)
    }

    // For hpos
    pub async fn uninstall_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.uninstall' : {:?}", msg);

        let payload_buf = msg.payload.to_vec();
        let _workload_id = serde_json::from_slice::<String>(&payload_buf)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to UNinstall workload...
        // nix_uninstall_with(workload_id)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            desired: WorkloadState::Uninstalled,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(serde_json::to_vec(&status)?)
    }
}
