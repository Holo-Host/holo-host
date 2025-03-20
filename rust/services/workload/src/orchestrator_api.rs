/*
Current Endpoints & Managed Subjects:
    - `add_workload`: handles the "WORKLOAD.add" subject
    - `update_workload`: handles the "WORKLOAD.update" subject
    - `delete_workload`: handles the "WORKLOAD.delete" subject
    - `handle_db_insertion`: handles the "WORKLOAD.insert" subject // published by mongo<>nats connector
    - `handle_db_modification`: handles the "WORKLOAD.modify" subject // published by mongo<>nats connector
    - `handle_status_update`: handles the "WORKLOAD.handle_status_update" subject // published by hosting agent

TODO(refactor) discuss the following alternative model:
    - `COMMAND.CHANGE_WORKLOAD`: request changes to a workload
    - `EVENT.WORKLOAD.CHANGED`: information about events that factually happened, .e.g. during the handling of a command
*/

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use crate::{
    types::{HostIdJSON, WorkloadResult},
    TAG_MAP_PREFIX_ASSIGNED_HOST,
};
use anyhow::Result;
use async_nats::Message;
use bson::{self, doc, oid::ObjectId, to_document, Bson};
use core::option::Option::None;
use db_utils::{
    mongodb::{IntoIndexes, MongoCollection, MongoDbAPI, MutMetadata},
    schemas::{self, Host, Workload, WorkloadState, WorkloadStatus},
};
use hpos_hal::inventory::HoloInventory;
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
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
                    payload: Default::default(),
                };
                workload.status = status.clone();

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
        const SUBJECT: &str = "WORKLOAD.update";

        if msg.subject.to_string() != SUBJECT {
            return Err(ServiceError::Internal {
                message: format!(
                    "received message with subject {} when this method expects {SUBJECT}",
                    msg.subject
                ),
                context: None,
            });
        }
        log::debug!("Incoming message for {SUBJECT}");
        self.process_request(
            msg,
            WorkloadState::Updating,
            WorkloadState::Error,
            |mut workload: schemas::Workload| async move {
                let status = WorkloadStatus {
                    id: workload._id,
                    desired: WorkloadState::Updated,
                    actual: WorkloadState::Updating,
                    payload: Default::default(),
                };
                workload.status = status.clone();

                // Convert the workload to a bson document and submit to mongodb
                let updated_workload_doc = to_document(&workload).map_err(|e| {
                    ServiceError::internal(
                        e.to_string(),
                        Some("Failed to convert workload to document".to_string()),
                    )
                })?;

                // Update the workload record in the db collection
                let _update_result = self
                    .workload_collection
                    .update_one_within(
                        doc! { "_id":  workload._id },
                        UpdateModifications::Document(doc! { "$set": updated_workload_doc }),
                        false,
                    )
                    .await?;

                log::info!(
                    "Successfully updated workload. MongodDB Workload ID={:?}",
                    workload._id
                );
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status,
                        workload: Some(workload),
                    },
                    maybe_response_tags: None,
                })
            },
        )
        .await
    }

    pub async fn delete_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.delete'");
        self.process_request(
            msg,
            WorkloadState::Deleted,
            WorkloadState::Error,
            |workload_id: ObjectId| async move {
                // TODO: Disucss with Stefan - what is the reason for passing in the whole workload? This fn only requires the workload ID to take the action it requires...
                // let workload_id = if let Some(workload_id) = workload._id {
                //     workload_id
                // } else {
                //     return Ok(WorkloadApiResult {
                //         result: WorkloadResult {
                //             status: WorkloadStatus {
                //                 ..workload.status.clone()
                //             },
                //             workload: Some(workload),
                //         },
                //         maybe_response_tags: None,
                //     });
                // };

                let new_status = WorkloadStatus {
                    id: Some(workload_id),
                    actual: WorkloadState::Deleted,
                    desired: WorkloadState::Unlinked,
                    payload: Default::default(),
                };

                self.update_workload_status_in_db(new_status.clone())
                    .await?;

                log::info!(
                    "Successfully deleted workload. MongodDB Workload ID={:?}",
                    workload_id
                );
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: new_status,
                        workload: None,
                    },
                    maybe_response_tags: None,
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
            WorkloadState::Installed,
            WorkloadState::Error,
            |workload: schemas::Workload| async move {
                log::debug!("New workload to assign. Workload={:#?}", workload);

                // Perform sanity check to ensure workload is not already assigned to a host and if so, exit fn
                if !workload.assigned_hosts.is_empty() {
                    log::warn!("Attempted to assign host for new workload, but host already exists.");
                    return Ok(WorkloadApiResult {
                        result: WorkloadResult {
                            status: workload.status,
                            workload: None,
                        },
                        maybe_response_tags: None,
                    });
                }

                let workload_id = workload.clone()._id.ok_or_else(|| {
                    ServiceError::internal(
                        format!("No `_id` found for workload. Unable to proceed assigning a host. Workload={:?}", workload),
                        Some("Missing workload ID".to_string()),
                    )
                })?;

                // Call mongodb to get hosts that meet the capacity requirements
                let eligible_hosts = self
                    .find_hosts_meeting_workload_criteria(workload.clone(), None)
                    .await?;
                log::trace!(
                    "Eligible hosts for new workload. MongodDB Hosts={:?}",
                    eligible_hosts
                );

                // Add the assigned Workload ID to the eligible host records 
                let eligible_host_ids: Vec<ObjectId> = eligible_hosts.iter().map(|h| h._id).collect();
                let assigned_host_ids = self
                    .assign_workload_to_hosts(workload_id, eligible_host_ids, workload.min_hosts)
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to assign workload to hosts".to_string()),
                        )
                    })?;

                log::trace!(
                    "Assigned host ids for new workload. MongodDB Hosts={:?}",
                    assigned_host_ids
                );

                let new_status = WorkloadStatus {
                    id: Some(workload_id),
                    desired: WorkloadState::Running, // TODO: connect with stefan on appropriate desired state
                    actual: WorkloadState::Assigned,
                    payload: Default::default(),
                };


                // Add the assigned Host IDs to the Workload record
                self.assign_hosts_to_workload(assigned_host_ids.clone(), workload_id, new_status.clone())
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to assign hosts to workload".to_string()),
                        )
                    })?;

                // Create tag map with all assigned host ids
                // Nb: This is required to inform nats to publish message to these hosts with workload status..
                let mut tag_map: HashMap<String, String> = HashMap::new();
                for (index, host_id) in assigned_host_ids.iter().cloned().enumerate() {
                    let assigned_host = eligible_hosts.iter().find(|h| h._id == host_id).ok_or_else(|| ServiceError::internal("Error: Failed to locate host device id from assigned host ids.".to_string(), Some("Unable to forward workload to Host.".to_string())))?;

                    tag_map.insert(format!("{TAG_MAP_PREFIX_ASSIGNED_HOST}{}", index), assigned_host.device_id.to_string());
                }

                log::trace!("Forwarding subject tag map: {tag_map:?}");

                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: new_status,
                        workload: Some(workload),
                    },
                    maybe_response_tags: Some(tag_map),
                })
            },
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

        let workload_id = workload.clone()._id.ok_or_else(|| {
            ServiceError::internal(
                format!(
                    "No `_id` found for workload. Unable to proceed assigning a host. Workload={:?}",
                    workload
                ),
                Some("Missing workload ID".to_string()),
            )
        })?;

        let mut tag_map: HashMap<String, String> = HashMap::new();
        let log_msg = format!(
            "Workload update in DB successful. Fwding update to assigned hosts. workload_id={}",
            workload_id
        );

        // Match on state (updating or deleted) and handle each case
        let result = match workload.status.actual {
            WorkloadState::Updating => {
                log::trace!("Updated workload to handle. Workload={:#?}", workload);

                let hosts = self
                    .fetch_hosts_assigned_to_workload(workload_id)
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to fetch assigned hosts".to_string()),
                        )
                    })?;

                self.unlink_workload_from_hosts(workload_id)
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to remove workload from hosts".to_string()),
                        )
                    })?;

                let eligible_hosts = self
                    .find_hosts_meeting_workload_criteria(workload.clone(), Some(hosts))
                    .await?;
                log::trace!(
                    "Eligible hosts for new workload. MongodDB Hosts={:?}",
                    eligible_hosts
                );

                let eligible_host_ids: Vec<ObjectId> =
                    eligible_hosts.iter().map(|h| h._id).collect();
                let assigned_host_ids = self
                    .assign_workload_to_hosts(workload_id, eligible_host_ids, workload.min_hosts)
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to assign workload to hosts".to_string()),
                        )
                    })?;

                // IMP: It is very important that the workload state changes to a state that is not `WorkloadState::Updating`..
                // IMP: Otherwise, this change will cause the workload update to loop between the db stream modification reads and this handler..
                let new_status = WorkloadStatus {
                    id: None,
                    desired: WorkloadState::Running,
                    actual: WorkloadState::Updated,

                    payload: Default::default(),
                };
                self.assign_hosts_to_workload(
                    assigned_host_ids.clone(),
                    workload_id,
                    new_status.clone(),
                )
                .await
                .map_err(|e| {
                    ServiceError::internal(
                        e.to_string(),
                        Some("Failed to assign hosts to workload".to_string()),
                    )
                })?;

                log::info!(
                    "Assigned workload to hosts. Host_Ids={:?} Workload={:#?}",
                    assigned_host_ids,
                    workload_id
                );

                // Create tag map with all assigned host ids
                // Nb: This is required to inform nats to publish message to these hosts with workload status..
                for (index, host_id) in assigned_host_ids.iter().enumerate() {
                    let assigned_host = eligible_hosts
                        .iter()
                        .find(|h| &h._id == host_id)
                        .ok_or_else(|| {
                            ServiceError::internal(
                                "Error: Failed to locate host device id from assigned host ids."
                                    .to_string(),
                                Some("Unable to forward workload to Host.".to_string()),
                            )
                        })?;
                    tag_map.insert(
                        format!("{TAG_MAP_PREFIX_ASSIGNED_HOST}{}", index),
                        assigned_host.device_id.to_string(),
                    );
                }

                log::trace!("Forwarding subject tag map: {tag_map:?}");

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
            WorkloadState::Deleted => {
                log::trace!("Deleted workload to handle. Workload={:#?}", workload);
                // Fetch current hosts with `workload_id`` to know which
                // hosts to send uninstall workload request to...
                let hosts = self
                    .fetch_hosts_assigned_to_workload(workload_id)
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to fetch assigned hosts".to_string()),
                        )
                    })?;

                self.unlink_workload_from_hosts(workload_id)
                    .await
                    .map_err(|e| {
                        ServiceError::internal(
                            e.to_string(),
                            Some("Failed to remove workload from hosts".to_string()),
                        )
                    })?;

                let new_status = WorkloadStatus {
                    id: Some(workload_id),
                    desired: WorkloadState::Uninstalled,
                    actual: WorkloadState::Unlinked,

                    payload: Default::default(),
                };
                self.update_workload_status_in_db(new_status.clone())
                    .await?;

                let host_ids = hosts
                    .iter()
                    .map(|h| {
                        h._id.ok_or_else(|| {
                            ServiceError::internal(
                                "Host missing ID".to_string(),
                                Some("Database integrity error".to_string()),
                            )
                        })
                    })
                    .collect::<Result<Vec<ObjectId>, ServiceError>>()?;

                // Create tag map with all assigned host ids
                // Nb: This is required to inform nats to publish message to these hosts with workload status..
                for (index, host_pubkey) in host_ids.iter().enumerate() {
                    tag_map.insert(
                        format!("{TAG_MAP_PREFIX_ASSIGNED_HOST}{}", index),
                        host_pubkey.to_hex(),
                    );
                }

                log::info!("{} Hosts={:?}", log_msg, hosts);

                WorkloadApiResult {
                    result: WorkloadResult {
                        status: new_status,
                        workload: Some(workload),
                    },
                    maybe_response_tags: Some(tag_map),
                }
            }
            _ => WorkloadApiResult {
                // Catches all other cases wherein a record in the workload collection was modified (not created),
                // with a state other than "Updating" or "Deleted".
                // In this case, we don't want to do take any new action, so we return a default status without any updates or fowarding tags.
                result: WorkloadResult {
                    status: WorkloadStatus {
                        id: Some(workload_id),
                        desired: workload.status.desired,
                        actual: workload.status.actual,
                        payload: Default::default(),
                    },
                    workload: None,
                },
                maybe_response_tags: None,
            },
        };

        Ok(result)
    }

    // NB: Published by the Hosting Agent whenever the status of a workload changes
    // TODO(correctness): make sure the errors are caught and sent to somewhere relevant
    pub async fn handle_status_update(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let incoming_subject = msg.subject.clone();
        log::debug!("Incoming message for '{incoming_subject}'");

        let workload_status = Self::convert_msg_to_type::<WorkloadResult>(msg)?.status;
        log::trace!("Workload status to update. Status={:?}", workload_status);

        let workload_id = workload_status.id;
        self.update_workload_status_in_db(workload_status.clone())
            .await
            .map_err(|e| {
                ServiceError::internal(
                    e.to_string(),
                    Some(format!(
                        "Failed to update workload status in db. id={:?}",
                        workload_id
                    )),
                )
            })?;

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status: workload_status,
                workload: None,
            },
            maybe_response_tags: None,
        })
    }

    async fn update_workload_status_in_db(
        &self,
        workload_status: WorkloadStatus,
    ) -> Result<(), ServiceError> {
        let workload_status_id = workload_status.id.ok_or_else(|| {
            ServiceError::internal(
                "Failed to read ._id from record".to_string(),
                Some("Missing workload status ID".to_string()),
            )
        })?;

        let status_bson = bson::to_bson(&workload_status).map_err(|e| {
            ServiceError::internal(
                e.to_string(),
                Some("Failed to serialize workload status".to_string()),
            )
        })?;

        self.workload_collection
            .update_one_within(
                doc! { "_id": workload_status_id },
                UpdateModifications::Document(doc! { "$set": { "status": status_bson } }),
                workload_status.actual == WorkloadState::Deleted,
            )
            .await?;

        Ok(())
    }

    // Verifies that a host meets the workload criteria
    pub fn verify_host_meets_workload_criteria(
        &self,
        assigned_host_inventory: &HoloInventory,
        workload: &Workload,
    ) -> bool {
        let host_drive_capacity = assigned_host_inventory.drives.iter().fold(0, |mut acc, d| {
            if let Some(capacity) = d.capacity_bytes {
                acc += capacity as i64;
            }
            acc
        });
        if host_drive_capacity < workload.system_specs.capacity.drive {
            return false;
        }
        if assigned_host_inventory.cpus.len() < workload.system_specs.capacity.cores as usize {
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

    async fn unlink_workload_from_hosts(&self, workload_id: ObjectId) -> Result<()> {
        self.host_collection
            .update_many_within(
                doc! {},
                UpdateModifications::Document(
                    doc! { "$pull": { "assigned_workloads": workload_id } },
                ),
                false,
            )
            .await?;
        log::info!(
            "Unlinked workload from previous hosts. Workload={:#?}",
            workload_id
        );
        Ok(())
    }

    // Looks through existing hosts to find possible hosts for a given workload
    // returns the minimum number of hosts required for workload (based on `workload.min_hosts` field)
    async fn find_hosts_meeting_workload_criteria(
        &self,
        workload: Workload,
        maybe_existing_hosts: Option<Vec<Host>>,
    ) -> Result<Vec<HostIdJSON>, ServiceError> {
        let mut needed_host_count = workload.min_hosts;
        let mut still_eligible_host_ids: Vec<HostIdJSON> = vec![];
        let mut host_ids: Vec<HostIdJSON> = vec![];

        if let Some(hosts) = maybe_existing_hosts {
            still_eligible_host_ids = hosts
                .into_iter()
                .filter_map(|h| {
                    if self.verify_host_meets_workload_criteria(&h.inventory, &workload) {
                        let _id = h
                            ._id
                            .ok_or_else(|| {
                                ServiceError::internal(
                                    format!("No `_id` found for workload. Workload={:?}", workload),
                                    Some(
                                        "Unable to proceed verifying host eligibility.".to_string(),
                                    ),
                                )
                            })
                            .ok()?;

                        Some(HostIdJSON {
                            _id,
                            device_id: h.device_id,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            needed_host_count -= still_eligible_host_ids.len() as i32;
        }

        if needed_host_count > 0 {
            let pipeline = vec![
                doc! {
                    "$match": {
                        // verify there are enough system resources
                        "$expr": { "$gte": [{ "$sum": "$inventory.drive" }, Bson::Int64(workload.system_specs.capacity.drive)]},
                        "$expr": { "$gte": [{ "$size": "$inventory.cpus" }, Bson::Int64(workload.system_specs.capacity.cores)]},
                    }
                },
                doc! {
                    // the maximum number of hosts returned should be the minimum hosts required by workload
                    // sample randomized results and always return back at least 1 result
                    "$sample": { "size": std::cmp::max(needed_host_count, 1) }
                },
                doc! {
                    // only return the `host._id` and `host.device_id` fields
                    "$project": { "_id": 1, "device_id": 1 }
                },
            ];

            host_ids = self
                .host_collection
                .aggregate::<HostIdJSON>(pipeline)
                .await?;
            if host_ids.is_empty() {
                let err_msg = format!(
                    "Failed to locate a compatible host for workload. Workload_Id={:?}",
                    workload._id
                );
                return Err(ServiceError::internal(err_msg, None));
            } else if workload.min_hosts > host_ids.len() as i32 {
                log::warn!(
                    "Failed to locate the the min required number of hosts for workload. Workload_Id={:?}",
                    workload._id
                );
            }
        }

        host_ids.extend(still_eligible_host_ids);

        Ok(host_ids)
    }

    async fn assign_workload_to_hosts(
        &self,
        workload_id: ObjectId,
        eligible_host_ids: Vec<ObjectId>,
        needed_host_count: i32,
    ) -> Result<Vec<ObjectId>> {
        // NB: This will attempt to assign the hosts up to 5 times.. then exit loop with warning message
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
                        "$push": { "assigned_workloads": workload_id }
                    }),
                    false,
                )
                .await?;

            if updated_host_result.matched_count == unassigned_host_ids.len() as u64 {
                log::debug!(
                    "Successfully updated Host records with the new workload id {}. Host_IDs={:?} Update_Result={:?}",
                    workload_id,
                    eligible_host_ids,
                    updated_host_result
                );
                return Ok(eligible_host_ids);
            } else if exit_flag == 5 {
                let unassigned_host_hashset: HashSet<ObjectId> =
                    unassigned_host_ids.into_iter().collect();
                let assigned_host_ids: Vec<ObjectId> = eligible_host_ids
                    .into_iter()
                    .filter(|id| !unassigned_host_hashset.contains(id))
                    .collect();
                log::warn!(
                    "Exiting loop after 5 attempts. Only assigned {} of {} required hosts. Workload_ID={}, Assigned_Host_IDs={:?}",
                    assigned_host_ids.len(),
                    needed_host_count,
                    workload_id,
                    assigned_host_ids
                );
                return Ok(assigned_host_ids);
            }

            log::warn!("Failed to update all selected host records with workload_id. Reattempting to pair remaining hosts...");
            let unassigned_hosts = self
                .host_collection
                .get_many_from(doc! {
                    "_id": { "$in": eligible_host_ids.clone() },
                    // Currently we only allow a single workload per host
                    "assigned_workloads": { "$size": 0 }
                })
                .await?;

            unassigned_host_ids = unassigned_hosts.into_iter().filter_map(|h| h._id).collect();
            exit_flag += 1;
        }
    }

    async fn assign_hosts_to_workload(
        &self,
        assigned_host_ids: Vec<ObjectId>,
        workload_id: ObjectId,
        new_status: WorkloadStatus,
    ) -> Result<()> {
        self.workload_collection
            .update_one_within(
                doc! { "_id": workload_id },
                UpdateModifications::Document(doc! {
                    "$set": {
                        "status": bson::to_bson(&new_status)
                            .map_err(|e| ServiceError::internal(e.to_string(), None))?,
                        "assigned_hosts": assigned_host_ids
                    }
                }),
                false,
            )
            .await?;
        Ok(())
    }

    // Helper function to initialize mongodb collections
    async fn init_collection<T>(
        client: &MongoDBClient,
        collection_name: &str,
    ) -> Result<MongoCollection<T>>
    where
        T: Serialize
            + for<'de> Deserialize<'de>
            + Unpin
            + Send
            + Sync
            + Default
            + Debug
            + IntoIndexes
            + MutMetadata,
    {
        Ok(MongoCollection::<T>::new(client, schemas::DATABASE_NAME, collection_name).await?)
    }
}
