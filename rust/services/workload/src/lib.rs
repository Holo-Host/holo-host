/*
Service Name: WORKLOAD
Subject: "WORKLOAD.>"
Provisioning Account: WORKLOAD
Users: orchestrator & hpos
Endpoints & Managed Subjects:
- `add_workload`: handles the "WORKLOAD.add" subject
- `remove_workload`: handles the "WORKLOAD.remove" subject
- Partial: `handle_db_change`: handles the "WORKLOAD.handle_change" subject // the stream changed output by the mongo<>nats connector (stream eg: DB_COLL_CHANGE_WORKLOAD).
- TODO: `start_workload`: handles the "WORKLOAD.start.{{hpos_id}}" subject
- TODO: `send_workload_status`: handles the "WORKLOAD.send_status.{{hpos_id}}" subject
- TODO: `uninstall_workload`: handles the "WORKLOAD.uninstall.{{hpos_id}}" subject
*/

mod types;
use anyhow::Result;
use async_nats::Message;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use types::WorkloadPayloadTypes;
use std::{fmt::Debug, sync::Arc};
use util_libs::{db::{mongodb::{IntoIndexes, MongoCollection, MongoDbAPI}, schemas::{self, Host, Workload, WorkloadState, WorkloadStatus}}, nats_js_client};
use rand::seq::SliceRandom;
use std::future::Future;
use serde::{Deserialize, Serialize};
use bson::{self, doc, to_document};

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
        Ok(Self {
            workload_collection: Self::init_collection(client, schemas::WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
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
    
    /*******************************  For Orchestrator   *********************************/
    // For orchestrator
    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.add'");
        let (_, status) = self.process_db_request(
            msg,
            WorkloadState::Reported,
            WorkloadPayloadTypes::Workload,
            "insert_one_into".to_string(),
            |workload: schemas::Workload| async move {
                let workload_id = self.workload_collection.insert_one_into(workload.clone()).await?;
                log::info!("Successfully added workload. MongodDB Workload ID={:?}", workload_id);
                Ok((
                    Some(workload_id),
                    WorkloadStatus {
                        desired: WorkloadState::Reported,
                        actual: WorkloadState::Reported,
                    },
                ))
            },
            WorkloadState::Error,
        )
        .await?;
        Ok(serde_json::to_vec(&types::ApiResult(None, status))?)
    }

    // For orchestrator
    pub async fn update_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.update'");
        let (maybe_payload, status) = self.process_db_request(
            msg,
            WorkloadState::Running,
            WorkloadPayloadTypes::Workload,
            "update_one_within".to_string(),
            |workload: schemas::Workload| async move {
                let workload_query = doc! { "_id":  workload._id.clone() };
                let updated_workload = to_document(&workload)?;
                self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(updated_workload)).await?;
                log::info!("Successfully updated workload. MongodDB Workload ID={:?}", workload._id);
                Ok((
                    workload._id,
                    WorkloadStatus {
                        desired: WorkloadState::Reported,
                        actual: WorkloadState::Reported,
                    },
                ))
            },
            WorkloadState::Error,
        )
        .await?;
        if let Some(workload) = maybe_payload {
            return Ok(serde_json::to_vec(&types::ApiResult(workload._id, status))?);
        }
        Ok(serde_json::to_vec(&types::ApiResult(None, status))?)

    }

    // For orchestrator
    pub async fn remove_workload(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.remove'");
        let (maybe_payload, status) = self.process_db_request(
            msg,
            WorkloadState::Removed,
            WorkloadPayloadTypes::MongoDbId,
            "delete_one_from".to_string(),
            |workload_id: schemas::MongoDbId| async move {
                let workload_query = doc! { "_id":  workload_id.clone() };
                self.workload_collection.delete_one_from(workload_query).await?;
                log::info!(
                    "Successfully removed workload from the Workload Collection. MongodDB Workload ID={:?}",
                    workload_id
                );
                Ok((
                    Some(workload_id),
                    WorkloadStatus {
                        desired: WorkloadState::Removed,
                        actual: WorkloadState::Removed,
                    },
                ))
            },
            WorkloadState::Error,
        )
        .await?;
        if let Some(workload_id) = maybe_payload {
            return Ok(serde_json::to_vec(&types::ApiResult(Some(workload_id), status))?);
        }
        Ok(serde_json::to_vec(&types::ApiResult(None, status))?)
    }


    // For orchestrator
    // NB: This is the stream that is automatically published by the nats-db-connector
    pub async fn handle_db_insertion(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.insert'");

        let payload_buf = msg.payload.to_vec();
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);

        // 0. Fail Safe: exit early if the workload provided does not include a `_id` feild
        let workload_id = if let Some(id) = workload.clone()._id { id } else {
            let err_msg = format!("No `_id` found for workload.  Unable to proceed assigning a host. Workload={:?}", workload);
            log::error!("{}", err_msg);
            return Ok(serde_json::to_vec(&types::ApiResult(workload._id, WorkloadStatus {
                desired: WorkloadState::Assigned,
                actual: WorkloadState::Error(err_msg),
            }))?);
        };

        let success_status = WorkloadStatus {
            desired: WorkloadState::Assigned,
            actual: WorkloadState::Assigned,
        };        

        // 1. Perform sanity check to ensure workload is not already assigned to a host, and if so exit fn
        if !workload.assigned_hosts.is_empty() {
            log::warn!("Attempted to assign host for new workload, but host already exists.");
            return Ok(serde_json::to_vec(&types::ApiResult(Some(workload_id), success_status))?);
        }

        // 2. Otherwise call mongodb to get host collection to get hosts that meet the capacity requirements
        let host_filter = doc! {
            "remaining_capacity.cores": { "$gte": workload.system_specs.capacity.cores },      
            "remaining_capacity.memory": { "$gte": workload.system_specs.capacity.memory },
            "remaining_capacity.disk": { "$gte": workload.system_specs.capacity.disk }
        };
        let eligible_hosts = self.host_collection.get_many_from(host_filter).await?;
        log::info!("Eligible hosts for new workload. MongodDB Host IDs={:?}", eligible_hosts);

        // 3. If no host is currently assigned OR the current host has insufficient capacity,
        // randomly choose host/node
        let host = match eligible_hosts.choose(&mut rand::thread_rng()) {
            Some(h) => h,
            None => {
                // todo: Try to get another host up to 5 times, if fails thereafter, return error
                let err_msg = format!("Failed to locate an eligible host to support the required workload capacity. Workload={:?}", workload);
                log::error!("{}", err_msg);
                return Ok(serde_json::to_vec(&types::ApiResult(workload._id, WorkloadStatus {
                    desired: WorkloadState::Assigned,
                    actual: WorkloadState::Error(err_msg),
                }))?);
            }
        };

        // Note: The `_id` is an option because it is only generated upon the intial insertion of a record in
        // a mongodb collection. This also means that whenever a record is fetched from mongodb, it must have the `_id` feild.
        // Using `unwrap` is therefore safe.
        let host_id = host._id.to_owned().unwrap();
        
        // 4. Update the Workload Collection with the assigned Host ID
        let workload_query = doc! { "_id":  workload_id.clone() };
        let updated_workload = to_document(&Workload {
            assigned_hosts: vec![host_id],
            ..workload.clone()
        })?;
        let updated_workload_result = self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(updated_workload)).await?;
        log::info!(
            "Successfully added new workload into the Workload Collection. MongodDB Workload ID={:?}",
            updated_workload_result
        );
    
        // 5. Update the Host Collection with the assigned Workload ID
        let host_query = doc! { "_id":  host.clone()._id };
        let updated_host =  to_document(&Host {
            assigned_workloads: vec![workload_id.clone()],
            ..host.to_owned()
        })?;
        let updated_host_result = self.host_collection.update_one_within(host_query, UpdateModifications::Document(updated_host)).await?;
        log::info!(
            "Successfully added new workload into the Workload Collection. MongodDB Host ID={:?}",
            updated_host_result
        );

        // 6. Return status and host
        Ok(serde_json::to_vec(&types::ApiResult(Some(workload_id), success_status))?)
    }    

    // Zeeshan to take a look:
    // For orchestrator
    // NB: This is the stream that is automatically published by the nats-db-connector
    pub async fn handle_db_update(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.update'");

        let success_status = WorkloadStatus {
            desired: WorkloadState::Assigned,
            actual: WorkloadState::Assigned,
        };

        let payload_buf = msg.payload.to_vec();
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);

        // TODO: ...handle the use case for the update entry change stream 

        Ok(serde_json::to_vec(&success_status)?)
    } 

    // Zeeshan to take a look:
    // For orchestrator
    // NB: This is the stream that is automatically published by the nats-db-connector
    pub async fn handle_db_deletion(&self, msg: Arc<Message>) -> Result<Vec<u8>, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.delete'");

        let success_status = WorkloadStatus {
            desired: WorkloadState::Assigned,
            actual: WorkloadState::Assigned,
        };

        let payload_buf = msg.payload.to_vec();
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);

        // TODO: ...handle the use case for the delete entry change stream
        
        Ok(serde_json::to_vec(&success_status)?)
    }    

     /*******************************   For Hosting Agent   *********************************/
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

    // Helper function to process incoming messages for MongoDB ops
    async fn process_db_request<T, Fut>(
        &self,
        msg: Arc<Message>,
        desired_state: WorkloadState,
        payload_type_name: types::WorkloadPayloadTypes,
        op_name: String,
        db_op: impl Fn(T) -> Fut + Send + Sync,
        error_state: impl Fn(String) -> WorkloadState + Send + Sync,
    ) -> Result<(Option<T>, WorkloadStatus), anyhow::Error>
    where
        T: for<'de> Deserialize<'de> + Clone + Send + Sync + Debug + 'static,
        Fut: Future<Output = Result<(Option<schemas::MongoDbId>, WorkloadStatus), anyhow::Error>> + Send,
    {
        // 1. Deserialize payload into the expected type
        let payload: T = match serde_json::from_slice(&msg.payload) {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("Unable to deserialize payload. Expected type='{:?}' Error: {:?}", payload_type_name, e);
                log::error!("{}", err_msg);
                let status = WorkloadStatus {
                    desired: desired_state,
                    actual: error_state(err_msg),
                };
                return Ok((None, status));
            }
        };

        // 2. Process db operation for collection
        match db_op(payload.clone()).await {
            Ok((_, status)) => Ok((Some(payload), status)),
            Err(e) => {
                let err_msg = format!("Failed to process db operation for Workload Collection. Op={}, Payload={:?}, Error={:?}", op_name, payload, e);
                log::error!("{}", err_msg);
                let status = WorkloadStatus {
                    desired: desired_state,
                    actual: error_state(err_msg),
                };

                // 3. return response for stream
                Ok((Some(payload), status))
            }
        }
    }
}
