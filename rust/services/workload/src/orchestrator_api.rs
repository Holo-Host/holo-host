/*
Endpoints & Managed Subjects:
- `add_workload`: handles the "WORKLOAD.add" subject
- `remove_workload`: handles the "WORKLOAD.remove" subject
- Partial: `handle_db_change`: handles the "WORKLOAD.handle_change" subject // the stream changed output by the mongo<>nats connector (stream eg: DB_COLL_CHANGE_WORKLOAD).
- TODO: `start_workload`: handles the "WORKLOAD.start.{{hpos_id}}" subject
- TODO: `send_workload_status`: handles the "WORKLOAD.send_status.{{hpos_id}}" subject
- TODO: `uninstall_workload`: handles the "WORKLOAD.uninstall.{{hpos_id}}" subject
*/

use super::{types, WorkloadServiceApi};
use anyhow::Result;
use core::option::Option::None;
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use async_nats::Message;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use bson::{self, doc, to_document};
use util_libs::{
    nats_js_client::ServiceError,
    db::{
        mongodb::{IntoIndexes, MongoCollection, MongoDbAPI},
        schemas::{self, Host, Workload, WorkloadState, WorkloadStatus}
    }
};

#[derive(Debug, Clone)]
pub struct OrchestratorWorkloadApi {
    pub workload_collection: MongoCollection<schemas::Workload>,
    pub host_collection: MongoCollection<schemas::Host>,
    pub user_collection: MongoCollection<schemas::User>,
}

impl WorkloadServiceApi for OrchestratorWorkloadApi {}

impl OrchestratorWorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, schemas::WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
        })
    }

    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.add'");
        self.process_request(
            msg,
            WorkloadState::Reported,
            |workload: schemas::Workload| async move {
                let workload_id = self.workload_collection.insert_one_into(workload.clone()).await?;
                log::info!("Successfully added workload. MongodDB Workload ID={:?}", workload_id);
                let new_workload = schemas::Workload {
                    _id: Some(workload_id),
                    ..workload
                };
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: new_workload._id,
                        desired: WorkloadState::Reported,
                        actual: WorkloadState::Reported,
                    },
                    None
                ))
            },
            WorkloadState::Error,
        )
        .await
    }

    pub async fn update_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.update'");
        self.process_request(
            msg,
            WorkloadState::Running,
            |workload: schemas::Workload| async move {
                let workload_query = doc! { "_id":  workload._id.clone() };
                let updated_workload_doc = to_document(&workload).map_err(|e| ServiceError::Internal(e.to_string()))?;
                self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(updated_workload_doc)).await?;
                log::info!("Successfully updated workload. MongodDB Workload ID={:?}", workload._id);
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: workload._id,
                        desired: WorkloadState::Reported,
                        actual: WorkloadState::Reported,
                    },
                    None
                ))
            },
            WorkloadState::Error,
        )
        .await

    }

    pub async fn remove_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.remove'");
        self.process_request(
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
                        id: Some(workload_id),
                        desired: WorkloadState::Removed,
                        actual: WorkloadState::Removed,
                    },
                    None
                ))
            },
            WorkloadState::Error,
        )
        .await
    }

    // NB: Automatically published by the nats-db-connector
    pub async fn handle_db_insertion(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.insert'");
        self.process_request(
            msg,
            WorkloadState::Assigned,
            |workload: schemas::Workload| async move {
                log::debug!("New workload to assign. Workload={:#?}", workload);

                // 0. Fail Safe: exit early if the workload provided does not include an `_id` field
                let workload_id = if let Some(id) = workload.clone()._id { id } else {
                    let err_msg = format!("No `_id` found for workload.  Unable to proceed assigning a host. Workload={:?}", workload);
                    return Err(ServiceError::Internal(err_msg));
                };

                // 1. Perform sanity check to ensure workload is not already assigned to a host
                // ...and if so, exit fn
                // todo: check for to ensure assigned host *still* has enough capacity for updated workload
                if !workload.assigned_hosts.is_empty() {
                    log::warn!("Attempted to assign host for new workload, but host already exists.");
                    let mut tag_map: HashMap<String, String> = HashMap::new();
                    for (index, host_pubkey) in workload.assigned_hosts.into_iter().enumerate() {
                        tag_map.insert(format!("assigned_host_{}", index), host_pubkey);
                    }
                    return Ok(types::ApiResult( 
                        WorkloadStatus {
                            id: Some(workload_id),
                            desired: WorkloadState::Assigned,
                            actual: WorkloadState::Assigned,
                        },
                        Some(tag_map)
                    ));
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
                        return Err(ServiceError::Internal(err_msg));
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
                let updated_workload_doc = to_document(updated_workload).map_err(|e| ServiceError::Internal(e.to_string()))?;
                let updated_workload_result = self.workload_collection.update_one_within(workload_query, UpdateModifications::Document(updated_workload_doc)).await?;
                log::trace!(
                    "Successfully added new workload into the Workload Collection. MongodDB Workload ID={:?}",
                    updated_workload_result
                );
                
                // 5. Update the Host Collection with the assigned Workload ID
                let host_query = doc! { "_id":  host.clone()._id };
                let updated_host_doc =  to_document(&Host {
                    assigned_workloads: vec![workload_id.clone()],
                    ..host.to_owned()
                }).map_err(|e| ServiceError::Internal(e.to_string()))?;
                let updated_host_result = self.host_collection.update_one_within(host_query, UpdateModifications::Document(updated_host_doc)).await?;
                log::trace!(
                    "Successfully added new workload into the Workload Collection. MongodDB Host ID={:?}",
                    updated_host_result
                );
                let mut tag_map: HashMap<String, String> = HashMap::new();
                for (index, host_pubkey) in updated_workload.assigned_hosts.iter().cloned().enumerate() {
                    tag_map.insert(format!("assigned_host_{}", index), host_pubkey);
                }
                Ok(types::ApiResult(
                    WorkloadStatus {
                        id: Some(workload_id),
                        desired: WorkloadState::Assigned,
                        actual: WorkloadState::Assigned,
                    },
                    Some(tag_map)
                ))
        },
            WorkloadState::Error,
        )
        .await
    }

    // Zeeshan to take a look:
    // NB: Automatically published by the nats-db-connector
    pub async fn handle_db_modification(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.modify'");
        
        let workload = Self::convert_to_type::<schemas::Workload>(msg)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);
        
        // TODO: ...handle the use case for the update entry change stream 

        let success_status = WorkloadStatus {
            id: workload._id,
            desired: WorkloadState::Running,
            actual: WorkloadState::Running,
        };
        
        Ok(types::ApiResult(success_status, None))
    }

    // NB: Published by the Hosting Agent whenever the status of a workload changes
    pub async fn handle_status_update(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.handle_status_update'");

        let workload_status = Self::convert_to_type::<WorkloadStatus>(msg)?;
        log::trace!("Workload status to update. Status={:?}", workload_status);

        // TODO: ...handle the use case for the workload status update within the orchestrator
        
        Ok(types::ApiResult(workload_status, None))
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
