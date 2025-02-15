/*
Endpoints & Managed Subjects:
    - `add_workload`: handles the "WORKLOAD.add" subject
    - `update_workload`: handles the "WORKLOAD.update" subject
    - `remove_workload`: handles the "WORKLOAD.remove" subject
    - `handle_db_insertion`: handles the "WORKLOAD.insert" subject // published by mongo<>nats connector
    - `handle_db_modification`: handles the "WORKLOAD.modify" subject // published by mongo<>nats connector
    - `handle_status_update`: handles the "WORKLOAD.handle_status_update" subject // published by hosting agent
*/

use crate::types::WorkloadResult;

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use anyhow::{anyhow, Result};
use async_nats::Message;
use bson::{self, doc, to_document, DateTime};
use core::option::Option::None;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, sync::Arc};
// use rand::seq::SliceRandom;
use util_libs::{
    db::{
        mongodb::{IntoIndexes, MongoCollection, MongoDbAPI},
        schemas::{self, Host, Workload, WorkloadState, WorkloadStatus},
    },
    nats_js_client::ServiceError,
};

#[derive(Debug, Clone)]
pub struct OrchestratorWorkloadApi {
    pub workload_collection: MongoCollection<schemas::Workload>,
    pub host_collection: MongoCollection<schemas::Host>,
    pub user_collection: MongoCollection<schemas::User>,
    pub developer_collection: MongoCollection<schemas::Developer>,
}

impl WorkloadServiceApi for OrchestratorWorkloadApi {}

impl OrchestratorWorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, schemas::WORKLOAD_COLLECTION_NAME)
                .await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
            developer_collection: Self::init_collection(client, schemas::DEVELOPER_COLLECTION_NAME)
                .await?,
        })
    }

    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.add'");
        self.process_request(
            msg,
            WorkloadState::Reported,
            |workload: schemas::Workload| async move {
                let workload_id = self
                    .workload_collection
                    .insert_one_into(workload.clone())
                    .await?;
                log::info!(
                    "Successfully added workload. MongodDB Workload ID={:?}",
                    workload_id
                );
                let new_workload = schemas::Workload {
                    _id: Some(workload_id),
                    ..workload
                };
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: new_workload._id,
                            desired: WorkloadState::Reported,
                            actual: WorkloadState::Reported,
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            },
            WorkloadState::Error,
        )
        .await
    }

    pub async fn update_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.update'");
        self.process_request(
            msg,
            WorkloadState::Running,
            |mut workload: schemas::Workload| async move {
                let workload_query = doc! { "_id":  workload._id };

                // update workload updated_at
                workload.metadata.updated_at = Some(DateTime::now());

                let updated_workload_doc =
                    to_document(&workload).map_err(|e| ServiceError::Internal(e.to_string()))?;

                self.workload_collection
                    .update_one_within(
                        workload_query,
                        UpdateModifications::Document(doc! { "$set": updated_workload_doc }),
                    )
                    .await?;
                log::info!(
                    "Successfully updated workload. MongodDB Workload ID={:?}",
                    workload._id
                );
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: workload._id,
                            desired: WorkloadState::Reported,
                            actual: WorkloadState::Reported,
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            },
            WorkloadState::Error,
        )
        .await
    }

    pub async fn remove_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.remove'");
        self.process_request(
            msg,
            WorkloadState::Removed,
            |workload_id: schemas::MongoDbId| async move {
                let workload_query = doc! { "_id":  workload_id };
                self.workload_collection.update_one_within(
                    workload_query,
                    UpdateModifications::Document(doc! {
                        "$set": {
                            "metadata.is_deleted": true,
                            "metadata.deleted_at": DateTime::now()
                        }
                    })
                ).await?;
                log::info!(
                    "Successfully removed workload from the Workload Collection. MongodDB Workload ID={:?}",
                    workload_id
                );
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: Some(workload_id),
                            desired: WorkloadState::Removed,
                            actual: WorkloadState::Removed,
                        },
                        workload: None
                    },
                    maybe_response_tags: None
                })
            },
            WorkloadState::Error,
        )
        .await
    }

    // NB: Automatically published by the nats-db-connector
    pub async fn handle_db_insertion(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
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
                        tag_map.insert(format!("assigned_host_{}", index), host_pubkey.to_hex());
                    }

                    return Ok(WorkloadApiResult {
                        result: WorkloadResult {
                            status: WorkloadStatus {
                                id: Some(workload_id),
                                desired: WorkloadState::Assigned,
                                actual: WorkloadState::Assigned,
                            },
                            workload: None
                        },
                        maybe_response_tags: Some(tag_map)
                    });
                }

                // 2. Otherwise call mongodb to get host collection to get hosts that meet the capacity requirements
                let eligible_hosts = self.find_hosts_meeting_workload_criteria(workload.clone()).await?;
                // let host_filter = doc! {
                //     "remaining_capacity.cores": { "$gte": workload.system_specs.capacity.cores },      
                //     "remaining_capacity.memory": { "$gte": workload.system_specs.capacity.memory },
                //     "remaining_capacity.disk": { "$gte": workload.system_specs.capacity.disk }
                // };
                // let eligible_hosts = self.host_collection.get_many_from(host_filter).await? ;
                // log::debug!("Eligible hosts for new workload. MongodDB Host IDs={:?}", eligible_hosts);

                // // 3. Randomly choose host/node
                // let host = match eligible_hosts.choose(&mut rand::thread_rng()) {
                //     Some(h) => h,
                //     None => {
                //         // todo: Try to get another host up to 5 times, if fails thereafter, return error
                //         let err_msg = format!("Failed to locate an eligible host to support the required workload capacity. Workload={:?}", workload);
                //         return Err(ServiceError::Internal(err_msg));
                //     }
                // };

                // Note: The `_id` is an option because it is only generated upon the intial insertion of a record in
                // a mongodb collection. This also means that whenever a record is fetched from mongodb, it must have the `_id` field.
                // TODO: Fix host selection style
                let host = &eligible_hosts[0];
                let host_id = host._id
                    .to_owned()
                    .ok_or_else(|| ServiceError::Internal("Failed to read ._id from record".to_string()))?;

                // 4. Update the Workload Collection with the assigned Host ID
                let workload_query = doc! { "_id":  workload_id };
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
                    assigned_workloads: vec![workload_id],
                    ..host.to_owned()
                }).map_err(|e| ServiceError::Internal(e.to_string()))?;
                let updated_host_result = self.host_collection.update_one_within(host_query, UpdateModifications::Document(updated_host_doc)).await?;
                log::trace!(
                    "Successfully added new workload into the Workload Collection. MongodDB Host ID={:?}",
                    updated_host_result
                );
                let mut tag_map: HashMap<String, String> = HashMap::new();
                for (index, host_pubkey) in updated_workload.assigned_hosts.iter().cloned().enumerate() {
                    tag_map.insert(format!("assigned_host_{}", index), host_pubkey.to_hex());
                }
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: Some(workload_id),
                            desired: WorkloadState::Assigned,
                            actual: WorkloadState::Assigned,
                        },
                        workload: None
                    },
                    maybe_response_tags: Some(tag_map)
                })
        },
            WorkloadState::Error,
        )
        .await
    }

    // NB: Automatically published by the nats-db-connector
    // triggers on mongodb [workload] collection (update)
    pub async fn handle_db_modification(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.modify'");

        let workload = Self::convert_msg_to_type::<schemas::Workload>(msg)?;
        log::trace!("New workload to assign. Workload={:#?}", workload);

        // 1. remove workloads from existing hosts
        self.host_collection
            .collection
            .update_many(
                doc! {},
                doc! { "$pull": { "assigned_workloads": workload._id } },
            )
            .await
            .map_err(ServiceError::Database)?;

        log::info!(
            "Remove workload from previous hosts. Workload={:#?}",
            workload._id
        );

        if !workload.metadata.is_deleted {
            // 3. add workload to specific hosts
            self.host_collection
                .collection
                .update_one(
                    doc! { "_id": { "$in": workload.clone().assigned_hosts } },
                    doc! { "$push": { "assigned_workloads": workload._id } },
                )
                .await
                .map_err(ServiceError::Database)?;

            log::info!("Added workload to new hosts. Workload={:#?}", workload._id);
        } else {
            log::info!(
                "Skipping (reason: deleted) - Added workload to new hosts. Workload={:#?}",
                workload._id
            );
        }

        let status = WorkloadStatus {
            id: workload._id,
            desired: WorkloadState::Updating,
            actual: WorkloadState::Updating,
        };
        log::info!("Workload update successful. Workload={:#?}", workload._id);

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status,
                workload: Some(workload),
            },
            maybe_response_tags: None,
        })
    }

    // NB: Published by the Hosting Agent whenever the status of a workload changes
    pub async fn handle_status_update(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.handle_status_update'");

        let workload_status = Self::convert_msg_to_type::<WorkloadResult>(msg)?.status;
        log::trace!("Workload status to update. Status={:?}", workload_status);

        let workload_status_id = workload_status
            .id
            .ok_or_else(|| ServiceError::Internal("Failed to read ._id from record".to_string()))?;

        self.workload_collection
            .update_one_within(
                doc! {
                    "_id": workload_status_id
                },
                UpdateModifications::Document(doc! {
                    "$set": {
                        "state": bson::to_bson(&workload_status.actual)
                            .map_err(|e| ServiceError::Internal(e.to_string()))?
                    }
                }),
            )
            .await?;

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status: workload_status,
                workload: None,
            },
            maybe_response_tags: None,
        })
    }

    // looks through existing hosts to find possible hosts for a given workload
    // returns the minimum number of hosts required for workload
    async fn find_hosts_meeting_workload_criteria(
        &self,
        workload: Workload,
    ) -> Result<Vec<Host>, ServiceError> {
        let pipeline = vec![
            doc! {
                "$match": {
                    // verify there are enough system resources
                    "remaining_capacity.disk": { "$gte": workload.system_specs.capacity.disk },
                    "remaining_capacity.memory": { "$gte": workload.system_specs.capacity.memory },
                    "remaining_capacity.cores": { "$gte": workload.system_specs.capacity.cores },

                    // limit how many workloads a single host can have
                    "assigned_workloads": { "$lt": 1 }
                }
            },
            doc! {
                // the maximum number of hosts returned should be the minimum hosts required by workload
                // sample randomized results and always return back atleast 1 result
                "$sample": std::cmp::min(workload.min_hosts as i32, 1)
            },
        ];
        let results = self.host_collection.aggregate(pipeline).await?;
        if results.is_empty() {
            return Err(ServiceError::Internal(
                anyhow!(
                    "Could not find a compatible host for this workload={:#?}",
                    workload._id
                )
                .to_string(),
            ));
        }
        Ok(results)
    }

    // verifies if a host meets the workload criteria
    pub fn verify_host_meets_workload_criteria(
        &self,
        workload: Workload,
        assigned_host: Host,
    ) -> bool {
        if assigned_host.remaining_capacity.disk < workload.system_specs.capacity.disk {
            return false;
        }
        if assigned_host.remaining_capacity.memory < workload.system_specs.capacity.memory {
            return false;
        }
        if assigned_host.remaining_capacity.cores < workload.system_specs.capacity.cores {
            return false;
        }

        true
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
