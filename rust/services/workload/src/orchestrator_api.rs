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
use async_nats::Message;
use bson::{self, doc, oid::ObjectId, to_document, DateTime};
use core::option::Option::None;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};
use util_libs::{
    db::{
        mongodb::{IntoIndexes, MongoCollection, MongoDbAPI},
        schemas::{self, Host, Workload, WorkloadState, WorkloadStatus},
    },
    nats::types::ServiceError,
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
            WorkloadState::Error,
            |mut workload: schemas::Workload| async move {
                let mut status = WorkloadStatus {
                    id: None,
                    desired: WorkloadState::Running,
                    actual: WorkloadState::Reported,
                };
                workload.status = status.clone();
                workload.metadata.created_at = Some(DateTime::now());

                let workload_id = self.workload_collection.insert_one_into(workload).await?;
                status.id = Some(workload_id);

                log::info!(
                    "Successfully added workload. MongodDB Workload ID={:?}",
                    workload_id
                );

                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status,
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            },
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
            WorkloadState::Updating,
            WorkloadState::Error,
            |mut workload: schemas::Workload| async move {
                let status = WorkloadStatus {
                    id: workload._id,
                    desired: WorkloadState::Updated,
                    actual: WorkloadState::Updating,
                };

                workload.status = status.clone();
                workload.metadata.updated_at = Some(DateTime::now());

                // convert workload to document and submit to mongodb
                let updated_workload_doc =
                    to_document(&workload).map_err(|e| ServiceError::Internal(e.to_string()))?;

                self.workload_collection
                    .update_one_within(
                        doc! { "_id":  workload._id },
                        UpdateModifications::Document(doc! { "$set": updated_workload_doc }),
                    )
                    .await?;

                log::info!(
                    "Successfully updated workload. MongodDB Workload ID={:?}",
                    workload._id
                );
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status,
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            },
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
            WorkloadState::Error,
            |workload_id: ObjectId| async move {
                let status = WorkloadStatus {
                    id: Some(workload_id),
                    desired: WorkloadState::Uninstalled,
                    actual: WorkloadState::Removed,
                };

                let updated_status_doc = bson::to_bson(&status)
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;

                self.workload_collection.update_one_within(
                    doc! { "_id":  workload_id },
                    UpdateModifications::Document(doc! {
                        "$set": {
                            "metadata.is_deleted": true,
                            "metadata.deleted_at": DateTime::now(),
                            "status": updated_status_doc
                        }
                    })
                ).await?;
                log::info!(
                    "Successfully removed workload from the Workload Collection. MongodDB Workload ID={:?}",
                    workload_id
                );
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status,
                        workload: None
                    },
                    maybe_response_tags: None
                })
            },
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
            WorkloadState::Error,
            |workload: schemas::Workload| async move {
                log::debug!("New workload to assign. Workload={:#?}", workload);

                // 0. Fail Safe: exit early if the workload provided does not include an `_id` field
                let workload_id = if let Some(id) = workload.clone()._id { id } else {
                    let err_msg = format!("No `_id` found for workload.  Unable to proceed assigning a host. Workload={:?}", workload);
                    return Err(ServiceError::Internal(err_msg));
                };

                let status = WorkloadStatus {
                    id: Some(workload_id),
                    desired: WorkloadState::Running,
                    actual: WorkloadState::Assigned,
                };

                // 1. Perform sanity check to ensure workload is not already assigned to a host and if so, exit fn
                if !workload.assigned_hosts.is_empty() {
                    log::warn!("Attempted to assign host for new workload, but host already exists.");
                    return Ok(WorkloadApiResult {
                        result: WorkloadResult {
                            status,
                            workload: None
                        },
                        maybe_response_tags: None
                    });
                }

                // 2. Otherwise call mongodb to get host collection to get hosts that meet the capacity requirements
                // & randomly choose host(s)
                let eligible_host_ids = self.find_hosts_meeting_workload_criteria(workload.clone(), None).await?;
                log::debug!("Eligible hosts for new workload. MongodDB Host IDs={:?}", eligible_host_ids);

                // 3. Update the selected host records with the assigned Workload ID
                let assigned_host_ids = self.assign_workload_to_hosts(workload_id, eligible_host_ids, workload.min_hosts).await.map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 4. Update the Workload Collection with the assigned Host ID
                let new_status = WorkloadStatus {
                    id: None, // remove the id to avoid redundant saving of it in the db
                    ..status.clone()
                };
                self.assign_hosts_to_workload(assigned_host_ids.clone(), workload_id, new_status).await.map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 5. Create tag map with host ids to inform nats to publish message to these hosts with workload install status                
                let mut tag_map: HashMap<String, String> = HashMap::new();
                for (index, host_pubkey) in assigned_host_ids.iter().cloned().enumerate() {
                    tag_map.insert(format!("assigned_host_{}", index), host_pubkey.to_hex());
                }

                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status,
                        workload: Some(workload)
                    },
                    maybe_response_tags: Some(tag_map)
                })
        })
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

        // Fail Safe: exit early if the workload provided does not include an `_id` field
        let workload_id = if let Some(id) = workload.clone()._id {
            id
        } else {
            let err_msg = format!(
                "No `_id` found for workload.  Unable to proceed assigning a host. Workload={:?}",
                workload
            );
            return Err(ServiceError::Internal(err_msg));
        };

        let mut tag_map: HashMap<String, String> = HashMap::new();
        let log_msg = format!(
            "Workload update in DB successful. Fwding update to assigned hosts. workload_id={}",
            workload_id
        );

        // Match on state (updating or removed) and handle each case
        let result = match workload.status.actual {
            WorkloadState::Updating => {
                log::trace!("Updated workload to handle. Workload={:#?}", workload);
                // 1. Fetch current hosts
                let hosts = self
                    .fetch_hosts_assigned_to_workload(workload_id)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 2. Remove workloads from existing hosts
                self.remove_workload_from_hosts(workload_id)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 3. Find eligible hosts
                let eligible_host_ids = self
                    .find_hosts_meeting_workload_criteria(workload.clone(), Some(hosts))
                    .await?;
                log::debug!(
                    "Eligible hosts for new workload. MongodDB Host IDs={:?}",
                    eligible_host_ids
                );

                // 4. Update the selected host records with the assigned Workload ID
                let assigned_host_ids = self
                    .assign_workload_to_hosts(workload_id, eligible_host_ids, workload.min_hosts)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 5. Update the Workload Collection with the assigned Host ID
                // IMP: It is very important that the workload state changes to a state that is not `WorkloadState::Updating`,
                // IMP: ...otherwise, this change will cause the workload update to loop between the db stream modification reads and this handler
                let new_status = WorkloadStatus {
                    id: None,
                    desired: WorkloadState::Running,
                    actual: WorkloadState::Updated,
                };
                self.assign_hosts_to_workload(
                    assigned_host_ids.clone(),
                    workload_id,
                    new_status.clone(),
                )
                .await
                .map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 6. Create tag map with host ids to inform nats to publish message to these hosts with workload install status
                for (index, host_pubkey) in assigned_host_ids.iter().enumerate() {
                    tag_map.insert(format!("assigned_host_{}", index), host_pubkey.to_hex());
                }

                log::info!("Added workload to new hosts. Workload={:#?}", workload_id);

                WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: Some(workload_id),
                            ..new_status
                        },
                        workload: Some(workload),
                    },
                    maybe_response_tags: Some(tag_map),
                }
            }
            WorkloadState::Removed => {
                log::trace!("Removed workload to handle. Workload={:#?}", workload);
                // 1. Fetch current hosts with `workload_id`` to know which
                // hosts to send uninstall workload request to...
                let hosts = self
                    .fetch_hosts_assigned_to_workload(workload_id)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 2. Remove workloads from existing hosts
                self.remove_workload_from_hosts(workload_id)
                    .await
                    .map_err(|e| ServiceError::Internal(e.to_string()))?;

                // 3. Create tag map with host ids to inform nats to publish message to these hosts with workload install status
                let host_ids = hosts
                    .iter()
                    .map(|h| {
                        h._id
                            .ok_or_else(|| ServiceError::Internal("Error".to_string()))
                    })
                    .collect::<Result<Vec<ObjectId>, ServiceError>>()?;
                for (index, host_pubkey) in host_ids.iter().enumerate() {
                    tag_map.insert(format!("assigned_host_{}", index), host_pubkey.to_hex());
                }

                log::info!("{} Hosts={:?}", log_msg, hosts);

                WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: Some(workload_id),
                            desired: WorkloadState::Uninstalled,
                            actual: WorkloadState::Removed,
                        },
                        workload: Some(workload),
                    },
                    maybe_response_tags: Some(tag_map),
                }
            }
            _ => {
                // Catches all other cases wherein a record in the workload collection was modified (not created),
                // with a state other than "Updating" or "Removed".
                // In this case, we don't want to do take any new action, so we return a default status without any updates or frowarding tags.
                WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: Some(workload_id),
                            desired: workload.status.desired,
                            actual: workload.status.actual,
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                }
            }
        };

        Ok(result)
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
                        "status": bson::to_bson(&workload_status)
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

    // Verifies that a host meets the workload criteria
    fn verify_host_meets_workload_criteria(
        &self,
        assigned_host: &Host,
        workload: &Workload,
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

    async fn fetch_hosts_assigned_to_workload(&self, workload_id: ObjectId) -> Result<Vec<Host>> {
        Ok(self
            .host_collection
            .get_many_from(doc! { "assigned_workloads": workload_id })
            .await?)
    }

    async fn remove_workload_from_hosts(&self, workload_id: ObjectId) -> Result<()> {
        self.host_collection
            .inner
            .update_many(
                doc! {},
                doc! { "$pull": { "assigned_workloads": workload_id } },
            )
            .await
            .map_err(ServiceError::Database)?;
        log::info!(
            "Removed workload from previous hosts. Workload={:#?}",
            workload_id
        );
        Ok(())
    }

    // Looks through existing hosts to find possible hosts for a given workload
    // returns the minimum number of hosts required for workload
    async fn find_hosts_meeting_workload_criteria(
        &self,
        workload: Workload,
        maybe_existing_hosts: Option<Vec<Host>>,
    ) -> Result<Vec<ObjectId>, ServiceError> {
        let mut needed_host_count = workload.min_hosts;
        let mut still_eligible_host_ids: Vec<ObjectId> = vec![];

        if let Some(hosts) = maybe_existing_hosts {
            still_eligible_host_ids = hosts.into_iter()
                .filter_map(|h| {
                    if self.verify_host_meets_workload_criteria(&h, &workload) {
                        h._id.ok_or_else(|| {
                            ServiceError::Internal(format!(
                                "No `_id` found for workload. Unable to proceed verifying host eligibility. Workload={:?}",
                                workload
                            ))
                        }).ok()
                    } else {
                        None
                    }
                })
                .collect();
            needed_host_count -= still_eligible_host_ids.len() as u16;
        }

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
                // sample randomized results and always return back at least 1 result
                "$sample": std::cmp::min( needed_host_count as i32, 1),

                // only return the `host._id` feilds
                "$project": { "_id": 1 }
            },
        ];
        let host_ids = self.host_collection.aggregate::<ObjectId>(pipeline).await?;
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

        let mut eligible_host_ids = host_ids;
        eligible_host_ids.extend(still_eligible_host_ids);

        Ok(eligible_host_ids)
    }

    async fn assign_workload_to_hosts(
        &self,
        workload_id: ObjectId,
        eligible_host_ids: Vec<ObjectId>,
        needed_host_count: u16,
    ) -> Result<Vec<ObjectId>> {
        // NB: This will attempt to assign the hosts up to 5 times.. then exit loop with warning message
        let assigned_host_ids: Vec<ObjectId>;
        let mut unassigned_host_ids: Vec<ObjectId> = eligible_host_ids.clone();
        let mut exit_flag = 0;
        loop {
            let updated_host_result = self
                .host_collection
                .update_many_within(
                    doc! {
                        "_id": { "$in": unassigned_host_ids.clone() },
                        // Currently we only allow a single workload per host
                        "assigned_workloads": { "$size": 0 }
                    },
                    UpdateModifications::Document(doc! {
                        "$push": {
                            "assigned_workloads": workload_id
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
                let unassigned_host_hashset: HashSet<ObjectId> =
                    unassigned_host_ids.into_iter().collect();
                assigned_host_ids = eligible_host_ids
                    .into_iter()
                    .filter(|id| !unassigned_host_hashset.contains(id))
                    .collect();
                log::warn!("Exiting loop after 5 attempts to assign the workload to the min number of hosts.
                    Only able to assign {} hosts. Workload_ID={}, Assigned_Host_IDs={:?}",
                    needed_host_count,
                    workload_id,
                    assigned_host_ids
                );
                break;
            }

            log::warn!("Failed to update all selected host records with workload_id.");
            log::debug!("Fetching paired host records to see which one(s) still remain unassigned to workload...");
            let unassigned_hosts = self
                .host_collection
                .get_many_from(doc! {
                    "_id": { "$in": eligible_host_ids.clone() },
                    "assigned_workloads": { "$size": 0 }
                })
                .await?;

            unassigned_host_ids = unassigned_hosts
                .into_iter()
                .map(|h| h._id.unwrap_or_default())
                .collect();
            exit_flag += 1;
        }

        Ok(assigned_host_ids)
    }

    async fn assign_hosts_to_workload(
        &self,
        assigned_host_ids: Vec<ObjectId>,
        workload_id: ObjectId,
        new_status: WorkloadStatus,
    ) -> Result<()> {
        let updated_workload_result = self
            .workload_collection
            .update_one_within(
                doc! {
                    "_id": workload_id
                },
                UpdateModifications::Document(doc! {
                    "$set": [{
                        "status": bson::to_bson(&new_status)
                            .map_err(|e| ServiceError::Internal(e.to_string()))?
                        }, {
                        "assigned_hosts": assigned_host_ids
                    }]
                }),
            )
            .await;

        log::trace!(
            "Successfully added new workload into the Workload Collection. MongodDB Workload ID={:?}",
            updated_workload_result
        );

        Ok(())
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
