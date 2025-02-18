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
    pub developer_collection: MongoCollection<schemas::Developer>,
}

impl WorkloadServiceApi for OrchestratorWorkloadApi {}

impl OrchestratorWorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, schemas::WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, schemas::HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, schemas::USER_COLLECTION_NAME).await?,
            developer_collection: Self::init_collection(client, schemas::DEVELOPER_COLLECTION_NAME).await?,
        })
    }

    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
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
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: new_workload._id,
                            desired: WorkloadState::Reported,
                            actual: WorkloadState::Reported,
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

    pub async fn update_workload(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.update'");
        self.process_request(
            msg,
            WorkloadState::Running,
            |workload: schemas::Workload| async move {
                let workload_query = doc! { "_id":  workload._id.clone() };

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
                        workload: None
                    },
                    maybe_response_tags: None
                })
            },
            WorkloadState::Error,
        )
        .await

    }

    pub async fn remove_workload(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
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
    pub async fn handle_db_insertion(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
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

                // 1. Perform sanity check to ensure workload is not already assigned to a host and if so, exit fn
                if !workload.assigned_hosts.is_empty() {
                    log::warn!("Attempted to assign host for new workload, but host already exists.");

                    return Ok(WorkloadApiResult {
                        result: WorkloadResult {
                            status: WorkloadStatus {
                                id: Some(workload_id),
                                desired: WorkloadState::Assigned,
                                actual: WorkloadState::Assigned,
                            },
                            workload: None
                        },
                        maybe_response_tags: None
                    });
                }

                // 2. Otherwise call mongodb to get host collection to get hosts that meet the capacity requirements
                // & randomly choose host(s)
                let eligible_host_ids = self.find_hosts_meeting_workload_criteria(workload.clone()).await?;
                log::debug!("Eligible hosts for new workload. MongodDB Host IDs={:?}", eligible_host_ids);

                // 3. Update the selected host records with the assigned Workload ID
                // NB: This will attempt to assign the hosts up to 5 times.. then exit loop with warning message
                let assigned_host_ids: Vec<schemas::MongoDbId>;
                let mut unassigned_host_ids: Vec<schemas::MongoDbId> = eligible_host_ids.clone();
                let mut exit_flag = 0;
                loop {
                    let updated_host_result = self.host_collection
                        .update_many_within(
                        doc! {
                            "_id": { "$in": unassigned_host_ids.clone() },
                            // Currently we only allow a single workload per host
                            "assigned_workloads": { "$size": 0 }
                        },
                        UpdateModifications::Document(doc! {
                            "$set": {
                                // Currently we only allow a single workload per host
                                "assigned_workloads": vec![workload_id]
                            }
                        }),
                    )
                    .await?;

                    if updated_host_result.matched_count == unassigned_host_ids.len() as u64 {
                        log::debug!(
                            "Successfully updated Host records with the new workload id {}. Host_IDs={:?} Update_Result={:?}",
                            workload_id,
                            eligible_host_ids,
                            updated_host_result
                        );
                        assigned_host_ids = eligible_host_ids;
                        break;
                    } else if exit_flag == 5 {
                        let unassigned_host_hashset: HashSet<schemas::MongoDbId> = unassigned_host_ids.into_iter().collect();
                        assigned_host_ids =  eligible_host_ids.into_iter().filter(|id| !unassigned_host_hashset.contains(id)).collect();
                        log::warn!("Exiting loop after 5 attempts to assign the workload to the min number of hosts. Only able to assign {} hosts. Workload_ID={}, Assigned_Host_IDs={:?}",
                            workload.min_hosts,
                            workload_id,
                            assigned_host_ids
                        );
                        break;
                    }

                    log::warn!("Failed to update all selected host records with workload_id.");
                    log::debug!("Fetching paired host records to see which one(s) still remain unassigned to workload...");
                    let unassigned_hosts= self.host_collection.get_many_from(doc! {
                        "_id": { "$in": eligible_host_ids.clone() },
                        "assigned_workloads": { "$size": 0 }
                    }).await?;

                    unassigned_host_ids = unassigned_hosts.into_iter().map(|h| h._id.unwrap_or_default()).collect();
                    exit_flag += 1;
                }

                // 4. Update the Workload Collection with the assigned Host ID
                let updated_workload_result = self.workload_collection
                    .update_one_within(
                        doc! {
                            "_id": workload_id
                        },
                        UpdateModifications::Document(doc! {
                            "$set": [{
                                "state": bson::to_bson(&WorkloadState::Assigned)
                                    .map_err(|e| ServiceError::Internal(e.to_string()))?
                                }, {
                                "assigned_hosts": assigned_host_ids.clone()
                            }]
                        }),
                    )
                    .await?;

                log::trace!(
                    "Successfully added new workload into the Workload Collection. MongodDB Workload ID={:?}",
                    updated_workload_result
                );

                // 5. Create tag map with host ids to inform nats to publish message to these hosts with workload install status                
                let mut tag_map: HashMap<String, String> = HashMap::new();
                for (index, host_pubkey) in assigned_host_ids.iter().cloned().enumerate() {
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
            .inner
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
                .inner
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

    // Looks through existing hosts to find possible hosts for a given workload
    // returns the minimum number of hosts required for workload
    async fn find_hosts_meeting_workload_criteria(
        &self,
        workload: Workload,
    ) -> Result<Vec<schemas::MongoDbId>, ServiceError> {
        let pipeline = vec![
            doc! {
                "$match": {
                    // verify there are enough system resources
                    "$expr": { "$gte": [{ "$sum": "$inventory.drive" }, Bson::Int64(workload.system_specs.capacity.drive as i64)]},
                    "$expr": { "$gte": [{ "$size": "$inventory.cpus" }, Bson::Int64(workload.system_specs.capacity.cores)]},

                    // limit how many workloads a single host can have
                    "assigned_workloads": { "$lt": 1 }
                }
            },
            doc! {
                // the maximum number of hosts returned should be the minimum hosts required by workload
                // sample randomized results and always return back at least 1 result
                "$sample": std::cmp::min(workload.min_hosts as i32, 1),

                // only return the `host._id` feilds
                "$project": { "_id": 1 }
            },
        ];
        let host_ids = self
            .host_collection
            .aggregate::<schemas::MongoDbId>(pipeline)
            .await?;
        if host_ids.is_empty() {
            let err_msg = format!(
                "Failed to locate a compatible host for workload. Workload_Id={:?}",
                workload._id
            );
            return Err(ServiceError::Internal(err_msg));
        } else if workload.min_hosts > host_ids.len() as u16 {
            log::warn!(
                "Failed to locate the the min required number of hosts for workload. Workload_Id={:?}",
                workload._id
            );
        }
        Ok(host_ids)
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