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

pub mod types;

use anyhow::{anyhow, Result};
use async_nats::Message;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use std::{fmt::Debug, sync::Arc};
use util_libs::{db::{mongodb::{IntoIndexes, MongoCollection, MongoDbAPI}, schemas::{self, Host, Workload, WorkloadState, WorkloadStatus, MongoDbId}}, nats_js_client};
use rand::seq::SliceRandom;
use std::future::Future;
use serde::{Deserialize, Serialize};
use bson::{self, doc, to_document, DateTime};

pub const WORKLOAD_SRV_NAME: &str = "WORKLOAD";
pub const WORKLOAD_SRV_SUBJ: &str = "WORKLOAD";
pub const WORKLOAD_SRV_VERSION: &str = "0.0.1";
pub const WORKLOAD_SRV_DESC: &str = "This service handles the flow of Workload requests between the Developer and the Orchestrator, and between the Orchestrator and HPOS.";


#[derive(Debug, Clone)]
pub struct WorkloadApi {
    pub workload_collection: MongoCollection<schemas::Workload>,
    pub host_collection: MongoCollection<schemas::Host>,
    pub user_collection: MongoCollection<schemas::User>,
    pub developer_collection: MongoCollection<schemas::Developer>,
}

impl WorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, schemas::WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
            developer_collection: Self::init_collection(client, schemas::DEVELOPER_COLLECTION_NAME).await?,
        })
    }

    pub fn call<F, Fut>(
        &self,
        handler: F,
    ) -> nats_js_client::AsyncEndpointHandler<types::ApiResult>
    where
        F: Fn(WorkloadApi, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<types::ApiResult, anyhow::Error>> + Send + 'static,
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> nats_js_client::JsServiceResponse<types::ApiResult> {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }

    /*******************************  For Orchestrator   *********************************/
    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.add'");
        Ok(self.process_request(
            msg,
            WorkloadState::Reported,
            |workload: schemas::Workload| async move {
                let workload_id = self.workload_collection.insert_one_into(workload.clone()).await?;
                log::info!("Successfully added workload. MongodDB Workload ID={:?}", workload_id);
                let updated_workload = schemas::Workload {
                    _id: Some(MongoDbId::parse_str(workload_id)?),
                    ..workload
                };
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: match updated_workload._id {
                            Some(oid) => {
                                Some(oid.to_hex())
                            },
                            None => None
                        },
                        desired: WorkloadState::Reported,
                        actual: WorkloadState::Reported,
                    },
                    None
                ))
            },
            WorkloadState::Error,
        )
        .await)
    }

    pub async fn update_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.update'");
        Ok(self.process_request(
            msg,
            WorkloadState::Running,
            |workload: schemas::Workload| async move {
                let workload_query = doc! { "_id":  workload._id.clone() };
                let updated_workload = to_document(&workload)?;
                self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(doc!{ "$set": updated_workload })).await?;
                log::info!("Successfully updated workload. MongodDB Workload ID={:?}", workload._id);
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: match workload._id {
                            Some(oid) => {
                                Some(oid.to_hex())
                            },
                            None => None
                        },
                        desired: WorkloadState::Reported,
                        actual: WorkloadState::Reported,
                    },
                    None
                ))
            },
            WorkloadState::Error,
        )
        .await)

    }

    pub async fn remove_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.remove'");
        Ok(self.process_request(
            msg,
            WorkloadState::Removed,
            |workload_id: schemas::MongoDbId| async move {
                let workload_query = doc! { "_id":  workload_id.clone() };
                self.workload_collection.delete_one_from(workload_query).await?;
                log::info!(
                    "Successfully removed workload from the Workload Collection. MongodDB Workload ID={:?}",
                    workload_id
                );
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: Some(workload_id.to_hex()),
                        desired: WorkloadState::Removed,
                        actual: WorkloadState::Removed,
                    },
                    None
                ))
            },
            WorkloadState::Error,
        )
        .await)
    }

    // NB: Automatically published by the nats-db-connector
    pub async fn handle_db_insertion(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.insert'");
        Ok(self.process_request(
            msg,
            WorkloadState::Assigned,
            |workload: schemas::Workload| async move {
                log::debug!("New workload to assign. Workload={:#?}", workload);

                // 0. Fail Safe: exit early if the workload provided does not include an `_id` field
                let workload_id = if let Some(id) = workload.clone()._id { id } else {
                    let err_msg = format!("No `_id` found for workload.  Unable to proceed assigning a host. Workload={:?}", workload);
                    return Err(anyhow!(err_msg));
                };

                // 1. Perform sanity check to ensure workload is not already assigned to a host
                // ...and if so, exit fn
                // todo: check for to ensure assigned host *still* has enough capacity for updated workload
                if !workload.assigned_hosts.is_empty() {
                    log::warn!("Attempted to assign host for new workload, but host already exists.");
                    return Ok(types::ApiResult( 
                        WorkloadStatus {
                            id: Some(workload_id.to_hex()),
                            desired: WorkloadState::Assigned,
                            actual: WorkloadState::Assigned,
                        },
                        Some(
                            workload.assigned_hosts
                            .iter().map(|id| id.to_hex()).collect())
                        )
                    );
                }

                // 2. Otherwise call mongodb to get host collection to get hosts that meet the capacity requirements
                let host_filter = doc! {
                    "remaining_capacity.cores": { "$gte": workload.system_specs.capacity.cores },      
                    "remaining_capacity.memory": { "$gte": workload.system_specs.capacity.memory },
                    "remaining_capacity.disk": { "$gte": workload.system_specs.capacity.disk }
                };
                let eligible_hosts = self.host_collection.get_many_from(host_filter).await? ;
                log::debug!("Eligible hosts for new workload. MongodDB Host IDs={:?}", eligible_hosts);

                // 3. Randomly choose host/node
                let host = match eligible_hosts.choose(&mut rand::thread_rng()) {
                    Some(h) => h,
                    None => {
                        // todo: Try to get another host up to 5 times, if fails thereafter, return error
                        let err_msg = format!("Failed to locate an eligible host to support the required workload capacity. Workload={:?}", workload);
                        return Err(anyhow!(err_msg));
                    }
                };

                // Note: The `_id` is an option because it is only generated upon the intial insertion of a record in
                // a mongodb collection. This also means that whenever a record is fetched from mongodb, it must have the `_id` feild.
                // Using `unwrap` is therefore safe.
                let host_id = host._id.to_owned().unwrap();
                
                // 4. Update the Workload Collection with the assigned Host ID
                let workload_query = doc! { "_id":  workload_id.clone() };
                let updated_workload = &Workload {
                    assigned_hosts: vec![host_id],
                    ..workload.clone()
                };
                let updated_workload_doc = to_document(updated_workload)?;
                let updated_workload_result = self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(updated_workload_doc)).await?;
                log::trace!(
                    "Successfully added new workload into the Workload Collection. MongodDB Workload ID={:?}",
                    updated_workload_result
                );
                
                // 5. Update the Host Collection with the assigned Workload ID
                let host_query = doc! { "_id":  host.clone()._id };
                let updated_host_doc =  to_document(&Host {
                    assigned_workloads: vec![workload_id.clone().to_hex()],
                    ..host.to_owned()
                })?;
                let updated_host_result = self.host_collection.update_one_within(host_query, UpdateModifications::Document(updated_host_doc)).await?;
                log::trace!(
                    "Successfully added new workload into the Workload Collection. MongodDB Host ID={:?}",
                    updated_host_result
                );
     
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: Some(workload_id.to_hex()),
                        desired: WorkloadState::Assigned,
                        actual: WorkloadState::Assigned,
                    },
                    Some(
                        updated_workload.assigned_hosts.to_owned()
                        .iter().map(|host| host.to_hex()).collect()
                    )
                ))
        },
            WorkloadState::Error,
        )
        .await)
    }

    // NB: Automatically published by the nats-db-connector
    pub async fn handle_db_update(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.update'");
        
        let payload_buf = msg.payload.to_vec();

        let mut workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("New workload to assign. Workload={:#?}", workload.clone());

        // 1. fetch existing workload document
        match self.workload_collection.get_one_from(doc! { "_id":  workload._id.clone() }).await?{
            Some(workload_doc) => {
                // 2. remove workload from hosts
                self.host_collection.update_one_within(
                    doc!{ "_id": { "$in": workload_doc.assigned_hosts } },
                    UpdateModifications::Document(doc!{ "$pull": { "assigned_workloads": workload_doc._id.clone() } })
                ).await?;
                
                // 3. remove workload from developers
                self.developer_collection.update_one_within(
                    doc!{ "_id": { "$in": workload_doc.assigned_developer } },
                    UpdateModifications::Document(doc!{ "$pull": { "assigned_workloads": workload_doc._id.clone() } })
                ).await?;
            },
            None => {
                log::warn!("Workload not found. Unable to remove workload.");
            }
        }

        // 4. add workload to hosts
        self.host_collection.update_one_within(
            doc!{ "_id": { "$in": workload.clone().assigned_hosts } },
            UpdateModifications::Document(doc!{ "$push": { "assigned_workloads": workload._id.clone() } })
        ).await?;
        
        // 5. add workload to developers
        self.developer_collection.update_one_within(
            doc!{ "_id": { "$in": workload.clone().assigned_developer } },
            UpdateModifications::Document(doc!{ "$push": { "assigned_workloads": workload._id.clone() } })
        ).await?;

        workload.updated_at = Some(DateTime::from_chrono(chrono::Utc::now()));

        let success_status = WorkloadStatus {
            id: match workload._id.clone() {
                Some(oid) => Some(oid.to_hex()),
                None => None
            },
            desired: WorkloadState::Updating,
            actual: WorkloadState::Updating,
        };
        
        Ok(types::ApiResult(success_status, None))
    }
  
    // NB: This is the stream that is automatically published by the nats-db-connector
    pub async fn handle_db_deletion(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {

        log::debug!("Incoming message for 'WORKLOAD.delete'");
        
        let payload_buf = msg.payload.to_vec();
        let workload: schemas::Workload = serde_json::from_slice(&payload_buf)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);
        
        // TODO: ...handle the use case for the delete entry change stream

        let success_status = WorkloadStatus {
            id: match workload._id {
                Some(oid) => Some(oid.to_hex()),
                None => None
            },
            desired: WorkloadState::Removed,
            actual: WorkloadState::Removed,
        };
        
        Ok(types::ApiResult(success_status, None))
    }

    // NB: Published by the Hosting Agent whenever the status of a workload changes
    pub async fn handle_status_update(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.read_status_update'");

        let payload_buf = msg.payload.to_vec();
        let workload_status: WorkloadStatus = serde_json::from_slice(&payload_buf)?;
        log::trace!("Workload status to update. Status={:?}", workload_status);

        // 1. fetch the workload document
        match self.workload_collection.get_one_from(doc! { "_id": workload_status.id.clone() }).await?{
            Some(workload_doc) => {
                // 2. remove workload from hosts
                self.host_collection.update_one_within(
                    doc!{ "_id": { "$in": workload_doc.assigned_hosts } },
                    UpdateModifications::Document(doc!{ "$pull": { "assigned_workloads": workload_doc._id.clone() } })
                ).await?;
                
                // 3. remove workload from developers
                self.developer_collection.update_one_within(
                    doc!{ "_id": { "$in": workload_doc.assigned_developer } },
                    UpdateModifications::Document(doc!{ "$pull": { "assigned_workloads": workload_doc._id.clone() } })
                ).await?;
            },
            None => {
                log::warn!("Workload not found. Unable to remove workload.");
            }
        }

        // 4. mark workload as deleted
        self.workload_collection.update_one_within(
            doc! { "_id":  workload_status.id.clone() },
            UpdateModifications::Document(doc!{ "$set": {
                "deleted": true,
                "deletedAt": DateTime::from_chrono(chrono::Utc::now())
            }
        })).await?;
        
        Ok(types::ApiResult(workload_status, None))
    }    

     /*******************************   For Host Agent   *********************************/
    pub async fn start_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.start' : {:?}", msg);

        let payload_buf = msg.payload.to_vec();
        let workload = serde_json::from_slice::<schemas::Workload>(&payload_buf)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to install workload...
        // eg: nix_install_with(workload)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            id: match workload._id {
                Some(oid) => Some(oid.to_hex()),
                None => None
            },
            desired: WorkloadState::Running,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(types::ApiResult(status, None))
    }

    pub async fn uninstall_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!("Incoming message for 'WORKLOAD.uninstall' : {:?}", msg);

        let payload_buf = msg.payload.to_vec();
        let workload_id = serde_json::from_slice::<String>(&payload_buf)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to UNinstall workload...
        // nix_uninstall_with(workload_id)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            id: Some(workload_id),
            desired: WorkloadState::Uninstalled,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(types::ApiResult(status, None))
    }

    // For host agent ? or elsewhere ?
    // TODO: Talk through with Stefan
    pub async fn send_workload_status(&self, msg: Arc<Message>) -> Result<types::ApiResult, anyhow::Error> {
        log::debug!(
            "Incoming message for 'WORKLOAD.send_workload_status' : {:?}",
            msg
        );

        let payload_buf = msg.payload.to_vec();
        let workload_status = serde_json::from_slice::<WorkloadStatus>(&payload_buf)?;

        // Send updated status:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(types::ApiResult(workload_status, None))
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

    // Helper function to streamline the processing of incoming workload messages
    // NB: Currently used to process requests for MongoDB ops and the subsequent db change streams these db edits create (via the mongodb<>nats connector)
    async fn process_request<T, Fut>(
        &self,
        msg: Arc<Message>,
        desired_state: WorkloadState,
        cb_fn: impl Fn(T) -> Fut + Send + Sync,
        error_state: impl Fn(String) -> WorkloadState + Send + Sync,
    ) -> types::ApiResult
    where
        T: for<'de> Deserialize<'de> + Clone + Send + Sync + Debug + 'static,
        Fut: Future<Output = Result<types::ApiResult, anyhow::Error>> + Send,
    {
        // 1. Deserialize payload into the expected type
        let payload: T = match serde_json::from_slice(&msg.payload) {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("Failed to deserialize payload for Workload Service Endpoint. Subject={} Error={:?}", msg.subject, e);
                log::error!("{}", err_msg);
                let status = WorkloadStatus {
                    id: None,
                    desired: desired_state,
                    actual: error_state(err_msg),
                };
                return types::ApiResult(status, None);
            }
        };

        // 2. Call callback handler
        match cb_fn(payload.clone()).await {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("Failed to process Workload Service Endpoint. Subject={} Payload={:?}, Error={:?}", msg.subject, payload, e);
                log::error!("{}", err_msg);
                let status = WorkloadStatus {
                    id: None,
                    desired: desired_state,
                    actual: error_state(err_msg),
                };

                // 3. return response for stream
                types::ApiResult(status, None)
            }
        }
    }
}
