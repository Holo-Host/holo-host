/*
Current Endpoints & Managed Subjects:
    - `add_workload`: handles the "WORKLOAD.add" subject
    - `update_workload`: handles the "WORKLOAD.update" subject
    - `delete_workload`: handles the "WORKLOAD.delete" subject
    - `manage_workload_on_host`: handles the "WORKLOAD.insert" subject
    - `handle_status_update`: handles the "WORKLOAD.handle_status_update" subject // published by hosting agent
*/

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use crate::{
    types::{HostIdJSON, WorkloadResult},
    TAG_MAP_PREFIX_ASSIGNED_HOST,
};
use anyhow::Result;
use async_nats::jetstream::Context;
use async_nats::Message;
use bson::{self, doc, oid::ObjectId, to_document};
use core::option::Option::None;
use db_utils::{
    mongodb::{
        api::MongoDbAPI,
        collection::MongoCollection,
        traits::{IntoIndexes, MutMetadata},
    },
    schemas::{
        host::{Host, HOST_COLLECTION_NAME},
        user::{User, USER_COLLECTION_NAME},
        workload::{Workload, WorkloadState, WorkloadStatus, WORKLOAD_COLLECTION_NAME},
        DATABASE_NAME,
    },
};
use futures::StreamExt;
use hpos_hal::inventory::HoloInventory;
use mongodb::{
    bson::Timestamp,
    options::{FullDocumentBeforeChangeType, FullDocumentType, UpdateModifications},
    Client as MongoDBClient,
};
use nats_utils::types::GetHeaderMap;
use nats_utils::types::GetResponse;
use nats_utils::types::GetSubjectTags;
use nats_utils::types::{ResponseSubjectsGenerator, ServiceError};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct OrchestratorWorkloadApi {
    pub workload_collection: MongoCollection<Workload>,
    pub host_collection: MongoCollection<Host>,
    pub user_collection: MongoCollection<User>,
}

impl WorkloadServiceApi for OrchestratorWorkloadApi {}

impl OrchestratorWorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, USER_COLLECTION_NAME).await?,
        })
    }

    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.add'");
        self.process_request(
            msg,
            WorkloadState::Reported,
            WorkloadState::Error,
            |mut workload: Workload| async move {
                let workload_id = workload._id;
                let mut status = WorkloadStatus {
                    // nb: avoid populating the status id before inserting into the db to avoid duplicate id references
                    id: None,
                    desired: WorkloadState::Running,
                    actual: WorkloadState::Reported,
                    payload: Default::default(),
                };
                workload.status = status.clone();
                self.workload_collection.insert_one_into(workload).await?;

                log::info!(
                    "Successfully added workload. MongodDB Workload ID={:?}",
                    workload_id
                );

                status.id = Some(workload_id);

                Ok(WorkloadApiResult {
                    result: WorkloadResult::Status(status),
                    maybe_response_tags: None,
                    maybe_headers: None,
                })
            },
        )
        .await
    }

    pub async fn update_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for {}", &msg.subject);

        self.process_request(
            msg,
            WorkloadState::Updated,
            WorkloadState::Error,
            |mut workload: Workload| async move {
                let workload_id = workload._id;
                let mut status = WorkloadStatus {
                    id: None,
                    desired: WorkloadState::Running,
                    actual: WorkloadState::Updated,
                    payload: Default::default(),
                };

                workload.status = status.clone();

                // convert workload to document and submit to mongodb
                let updated_workload_doc = to_document(&workload).map_err(|e| {
                    ServiceError::internal(
                        e.to_string(),
                        Some("Failed to convert workload to document".to_string()),
                    )
                })?;

                let _update_result = self
                    .workload_collection
                    .update_one_within(
                        doc! { "_id":  workload_id },
                        UpdateModifications::Document(doc! { "$set": updated_workload_doc }),
                        false,
                    )
                    .await?;

                log::info!(
                    "Successfully updated workload. MongodDB Workload ID={:?}",
                    workload._id
                );

                status.id = Some(workload_id);

                Ok(WorkloadApiResult {
                    result: WorkloadResult::Status(status),
                    maybe_response_tags: None,
                    maybe_headers: None,
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
            |workload: Workload| async move {
                let mut status = WorkloadStatus {
                    id: None,
                    desired: WorkloadState::Uninstalled,
                    actual: WorkloadState::Deleted,
                    payload: Default::default(),
                };

                let updated_status_doc = bson::to_bson(&status).map_err(|e| {
                    ServiceError::internal(
                        e.to_string(),
                        Some("Failed to serialize workload status".to_string()),
                    )
                })?;

                self.workload_collection
                    .update_one_within(
                        doc! { "_id":  workload._id },
                        UpdateModifications::Document(doc! {
                            "$set": {
                                "status": updated_status_doc
                            }
                        }),
                        true,
                    )
                    .await?;

                log::info!(
                    "Successfully deleted workload. MongodDB Workload ID={:?}",
                    workload._id
                );

                status.id = Some(workload._id);

                Ok(WorkloadApiResult {
                    result: WorkloadResult::Status(status),
                    maybe_response_tags: None,
                    maybe_headers: None,
                })
            },
        )
        .await
    }

    // NB: Published by the Hosting Agent whenever the status of a workload changes
    // TODO(correctness): make sure the errors are caught and sent to somewhere relevant
    pub async fn handle_status_update(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming status update message.");
        let maybe_headers = msg.headers.clone();

        let workload_status = match Self::convert_msg_to_type::<WorkloadResult>(msg)? {
            WorkloadResult::Status(mut status) => {
                if status.id.is_none() {
                    let err_msg = ServiceError::internal(
                        "Received invalid workload update message. err=No workload status id found in message or headers.".to_string(),
                        Some("WorkloadApiError::handle_status_update".to_string()),
                    );
                    let headers = maybe_headers.as_ref().ok_or(err_msg.clone())?;
                    let workload_id_str = headers
                        .get("workload_id")
                        .ok_or(err_msg.clone())?
                        .to_string();
                    let workload_id = ObjectId::parse_str(&workload_id_str).map_err(|_| err_msg)?;
                    status.id = Some(workload_id);
                }
                status
            }
            WorkloadResult::Workload(mut workload) => {
                if workload.status.id.is_none() {
                    workload.status.id = Some(workload._id);
                }
                workload.status
            }
        };

        log::debug!(
            "Received workload status update. Status={:?}",
            workload_status
        );

        // Remove the workload from the hosts if the workload binary is now uninstalled
        // NB: unwrap is safe here because we check if it is set above
        if workload_status.actual == WorkloadState::Uninstalled {
            self.remove_workload_from_hosts(workload_status.id.unwrap())
                .await?;
        }

        // Update the workload status in the db
        let status_bson = bson::to_bson(&workload_status).map_err(|e| {
            ServiceError::internal(
                e.to_string(),
                Some("Failed to serialize workload status".to_string()),
            )
        })?;

        // NB: unwrap is safe here because we check if it is set above
        self.workload_collection
            .update_one_within(
                doc! { "_id": workload_status.id.unwrap() },
                UpdateModifications::Document(doc! { "$set": { "status": status_bson } }),
                false,
            )
            .await?;

        Ok(WorkloadApiResult {
            result: WorkloadResult::Status(workload_status),
            maybe_response_tags: None,
            maybe_headers: None,
        })
    }

    pub async fn manage_workload_on_host(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.insert'");
        let workload = Self::convert_msg_to_type::<Workload>(msg)?;

        let workload_before_change = self
            .workload_collection
            .get_one_from(doc! { "_id": workload._id })
            .await
            .unwrap_or_default();

        match workload.status.desired {
            WorkloadState::Running | WorkloadState::Deleted | WorkloadState::Uninstalled => {
                self.handle_workload_change_event(workload, workload_before_change)
                    .await
            }
            _ => Err(ServiceError::internal(
                "Received invalid desired state for host update.".to_string(),
                Some("WorkloadApiError::manage_workload_on_host".to_string()),
            )),
        }
    }

    async fn handle_workload_change_event(
        &self,
        workload: Workload,
        workload_before_change: Option<Workload>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Match on state and handle each case
        match workload.status.actual {
            WorkloadState::Reported => {
                log::debug!("Detected new workload to assign. Workload={:#?}", workload);
                self.handle_workload_assignment(workload, 0).await
            }
            WorkloadState::Updated => {
                log::trace!(
                    "Detected workload updated to handle. Workload={:#?}",
                    workload
                );
                self.handle_workload_update(workload, workload_before_change)
                    .await
            }
            WorkloadState::Deleted => {
                log::trace!(
                    "Detected workload deletion to handle. Workload={:#?}",
                    workload
                );
                self.handle_workload_deletion(workload).await
            }
            _ => {
                // Catches all other cases wherein a record in the workload collection was modified
                // with a state other than "Reported", "Updating", or "Deleted".
                let valid_events = vec![
                    WorkloadState::Reported,
                    WorkloadState::Updated,
                    WorkloadState::Deleted,
                ];
                log::warn!("Received invalid actual state for workload change event. valid_events={:?}, workload={:#?}", valid_events, workload);
                Ok(WorkloadApiResult {
                    result: WorkloadResult::Status(WorkloadStatus {
                        id: Some(workload._id),
                        ..workload.status
                    }),
                    maybe_response_tags: None,
                    maybe_headers: None,
                })
            }
        }
    }

    async fn handle_workload_assignment(
        &self,
        workload: Workload,
        num_hosts_to_add: i32,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::info!("Orchestrator::handle_workload_assignment");

        // Find minimum number of eligible hosts for the new workload
        let min_eligible_hosts = self
            .get_min_random_hosts_for_workload(workload.clone(), num_hosts_to_add)
            .await?;

        log::debug!(
            "Eligible hosts for new workload. MongodDB Hosts={:?}",
            min_eligible_hosts
        );

        // Assign workload to hosts and create response
        self.assign_workload_and_create_response(workload, min_eligible_hosts)
            .await
    }

    async fn handle_workload_update(
        &self,
        workload: Workload,
        workload_before_change: Option<Workload>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::info!("Orchestrator::handle_workload_update");

        let mut num_hosts_to_add = 0;

        if let Some(workload_before_change) = workload_before_change {
            log::trace!(
                "Full document before change is available. workload_before_change={:?}",
                workload_before_change
            );

            if workload.manifest == workload_before_change.manifest
                && workload.system_specs == workload_before_change.system_specs
            {
                log::info!(
                    "Neither the Workload manifest nor the system specs have changed. Skipping reassignment and any update of the workload in hosts. workload={:?}, workload_before_change={:?}",
                    workload, workload_before_change
                );

                return Ok(WorkloadApiResult {
                    result: WorkloadResult::Status(WorkloadStatus {
                        actual: WorkloadState::Running,
                        ..workload.status
                    }),
                    maybe_response_tags: None,
                    maybe_headers: None,
                });
            }

            if workload.min_hosts > workload_before_change.min_hosts {
                log::info!(
                    "The workload min_hosts has increased. Adding hosts. current_min_hosts={:?}, prior_min_hosts={:?}",
                    workload.min_hosts, workload_before_change.min_hosts
                );
                num_hosts_to_add = workload.min_hosts - workload_before_change.min_hosts;
            }
        };

        // IMP: We are not handling the host removal case here - ie: whenever the workload min_hosts has decreased.
        // TODO: Discuss with team how we want to handle the removal case
        // Should the hosts chosen for removal be randomized, or should we rely on host capacity or other criteria?
        if num_hosts_to_add > 0 {
            self.handle_workload_assignment(workload, num_hosts_to_add)
                .await
        } else {
            // Fetch current hosts and remove workload from them
            self.remove_workload_from_hosts(workload._id).await?;
            self.handle_workload_assignment(workload, 0).await
        }
    }

    // TODO: Only delete/unpair hosts from workload collection upon receiving uninsalled confirmation back frlm hos
    async fn handle_workload_deletion(
        &self,
        workload: Workload,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Fetch current hosts and remove workload from them
        let hosts = self.fetch_hosts_assigned_to_workload(workload._id).await?;
        self.remove_workload_from_hosts(workload._id).await?;

        let new_status = WorkloadStatus {
            id: Some(workload._id),
            desired: WorkloadState::Uninstalled,
            actual: WorkloadState::Deleted,
            payload: Default::default(),
        };

        // Remove hosts from the workload and update status to uninstall from hosts
        // NB: We should not remove the workload from a given host collection until we recieve a successful uninstallation message from the host
        let empty_hosts = vec![];
        self.assign_hosts_to_workload(empty_hosts, new_status.clone())
            .await?;
        log::info!(
            "Workload update in DB successful. Fwding update to assigned hosts. workload_id={} Hosts={:?}",
            workload._id,
            hosts
        );
        // Create tag map for response
        let mut subject_tag_map = HashMap::new();
        for (index, host) in hosts.iter().enumerate() {
            let host_id = host._id.ok_or_else(|| {
                ServiceError::internal(
                    "Host missing ID".to_string(),
                    Some("Database integrity error".to_string()),
                )
            })?;
            subject_tag_map.insert(
                format!("{TAG_MAP_PREFIX_ASSIGNED_HOST}{}", index),
                host_id.to_hex(),
            );
        }
        log::trace!("Subject tag map: {subject_tag_map:?}");

        let mut header_map = async_nats::HeaderMap::new();
        header_map.insert("workload_id", workload._id.to_hex());
        log::trace!("Nats header map: {header_map:?}");

        Ok(WorkloadApiResult {
            result: WorkloadResult::Workload(Workload {
                status: new_status,
                ..workload
            }),
            maybe_response_tags: Some(subject_tag_map),
            maybe_headers: Some(header_map),
        })
    }

    async fn fetch_hosts_assigned_to_workload(
        &self,
        workload_id: ObjectId,
    ) -> Result<Vec<Host>, ServiceError> {
        self.host_collection
            .get_many_from(doc! { "assigned_workloads": workload_id })
            .await
    }

    async fn remove_workload_from_hosts(&self, workload_id: ObjectId) -> Result<(), ServiceError> {
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
            "Removed workload from previous hosts. Workload={:#?}",
            workload_id
        );
        Ok(())
    }

    // Looks through existing hosts to find possible hosts for a given workload
    // returns the minimum number of hosts required for workload
    async fn get_min_random_hosts_for_workload(
        &self,
        workload: Workload,
        num_hosts_to_add: i32,
    ) -> Result<Vec<HostIdJSON>, ServiceError> {
        let needed_host_count = if num_hosts_to_add > 0 {
            num_hosts_to_add
        } else {
            workload.min_hosts
        };

        let pipeline = vec![
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
        let host_ids = self
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

        Ok(host_ids)
    }

    async fn assign_hosts_to_workload(
        &self,
        assigned_host_ids: Vec<ObjectId>,
        new_status: WorkloadStatus,
    ) -> Result<(), ServiceError> {
        let workload_id = new_status.id.ok_or_else(|| {
            ServiceError::internal(
                "Workload ID is required to assign hosts to a workload".to_string(),
                Some("WorkloadApiError::assign_hosts_to_workload".to_string()),
            )
        })?;

        self.workload_collection
            .update_one_within(
                doc! { "_id": workload_id },
                UpdateModifications::Document(doc! {
                    "$set": {
                        "status": bson::to_bson(& WorkloadStatus{id: None, ..new_status})
                            .map_err(|e| ServiceError::internal(e.to_string(), None))?,
                        "assigned_hosts": assigned_host_ids
                    }
                }),
                false,
            )
            .await?;
        Ok(())
    }

    async fn assign_workload_to_hosts(
        &self,
        workload_id: ObjectId,
        hosts_to_assign: Vec<ObjectId>,
    ) -> Result<Vec<ObjectId>, ServiceError> {
        // NB: This will attempt to assign the hosts up to 5 times.. then exit loop with warning message
        let needed_host_count = hosts_to_assign.len() as u64;
        let mut unassigned_host_ids: Vec<ObjectId> = hosts_to_assign.clone();
        let mut error_count = 0;

        loop {
            let updated_host_result = self
                .host_collection
                .update_many_within(
                    doc! {
                        "_id": { "$in": unassigned_host_ids.clone() },
                    },
                    UpdateModifications::Document(doc! {
                        "$push": { "assigned_workloads": workload_id }
                    }),
                    false,
                )
                .await?;

            if updated_host_result.matched_count == needed_host_count {
                log::debug!(
                    "Successfully updated Host records with the new workload id {}. Host_IDs={:?} Update_Result={:?}",
                    workload_id,
                    hosts_to_assign,
                    updated_host_result
                );
                return Ok(hosts_to_assign);
            }

            unassigned_host_ids = self
                .host_collection
                .get_many_from(doc! {
                    "_id": { "$in": hosts_to_assign.clone() },
                    "assigned_workloads": { "$not": { "$elemMatch": { "$eq": workload_id } } }
                })
                .await?
                .into_iter()
                .filter_map(|h| h._id)
                .collect();

            if error_count >= 5 {
                let unassigned_host_hashset: HashSet<ObjectId> =
                    unassigned_host_ids.into_iter().collect();
                let assigned_host_ids: Vec<ObjectId> = hosts_to_assign
                    .into_iter()
                    .filter(|id| !unassigned_host_hashset.contains(id))
                    .collect();

                if assigned_host_ids.is_empty() {
                    return Err(ServiceError::internal(
                        format!("Failed to assign workload to any hosts. workload_id={workload_id}, needed_host_count={needed_host_count:?}"),
                        Some("WorkloadApiError::assign_workload_to_hosts".to_string()),
                    ));
                }

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
            error_count += 1;
        }
    }

    async fn assign_workload_and_create_response(
        &self,
        workload: Workload,
        min_eligible_hosts: Vec<HostIdJSON>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Assign workload to minimum required number of eligible hosts
        let min_eligible_host_ids: Vec<ObjectId> =
            min_eligible_hosts.iter().map(|h| h._id).collect();
        let assigned_host_ids = self
            .assign_workload_to_hosts(workload._id, min_eligible_host_ids)
            .await?;

        // Update workload status and assigned hosts
        let new_status = WorkloadStatus {
            id: Some(workload._id),
            desired: WorkloadState::Running,
            actual: WorkloadState::Assigned,
            payload: Default::default(),
        };

        self.assign_hosts_to_workload(assigned_host_ids.clone(), new_status.clone())
            .await?;

        // Create tag map for response
        let mut subject_tag_map = HashMap::new();
        for (index, host_id) in assigned_host_ids.iter().enumerate() {
            let assigned_host = min_eligible_hosts
                .iter()
                .find(|h| h._id == *host_id)
                .ok_or_else(|| {
                    ServiceError::internal(
                        "Error: Failed to locate host device id from assigned host ids."
                            .to_string(),
                        Some("Unable to forward workload to Host.".to_string()),
                    )
                })?;

            subject_tag_map.insert(
                format!("{TAG_MAP_PREFIX_ASSIGNED_HOST}{}", index),
                assigned_host.device_id.to_string(),
            );
        }

        if !subject_tag_map.is_empty() {
            log::info!(
                "Assigned workload to hosts. Workload={:#?}\nDeviceIds={:#?}",
                workload._id,
                subject_tag_map.values()
            );
        }
        log::trace!("Subject tag Map: {subject_tag_map:?}");

        let mut header_map = async_nats::HeaderMap::new();
        header_map.insert("workload_id", workload._id.to_hex());
        log::trace!("Nats header map: {header_map:?}");

        Ok(WorkloadApiResult {
            result: WorkloadResult::Workload(Workload {
                status: new_status,
                ..workload
            }),
            maybe_response_tags: Some(subject_tag_map),
            maybe_headers: Some(header_map),
        })
    }

    // NB: This is a baseline for actual matching logic. It is a scaffold for future.
    pub fn _verify_host_meets_workload_criteria(
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

    pub async fn publish_response_to_hosts(
        &self,
        jetstream: Context,
        service_subject: String,
        response_subject_fn: ResponseSubjectsGenerator,
        workload_api_result: WorkloadApiResult,
    ) {
        let response_bytes = workload_api_result.get_response();
        let header_map = workload_api_result.get_header_map();
        let response_subjects = response_subject_fn(workload_api_result.get_subject_tags());
        for response_subject in response_subjects.iter() {
            let subject = format!("{}.{}", service_subject, response_subject);
            log::debug!("publishing a message for hosts on {subject}");

            if let Err(err) = match header_map {
                Some(ref header_map) => {
                    jetstream
                        .publish_with_headers(
                            subject.clone(),
                            header_map.clone(),
                            response_bytes.clone(),
                        )
                        .await
                }
                None => {
                    jetstream
                        .publish(subject.clone(), response_bytes.clone())
                        .await
                }
            } {
                log::error!(
                    "WORKLOAD_API_LOG::Failed to publish new message to host: subj='{}', service={}, err={:?}",
                    subject,
                    "publish_response_to_hosts",
                    err,
                );
            };
        }
    }

    pub async fn stream_workload_changes(
        &self,
        jetstream: Context,
        service_subject: String,
        response_subject_fn: ResponseSubjectsGenerator,
    ) -> Result<(), ServiceError> {
        // Create change stream that retrieves get full document and starts at operation time
        let collection = self.workload_collection.inner.clone();

        // Track the last change event id for recovering change stream placement
        let mut last_resume_token: Option<mongodb::change_stream::event::ResumeToken> = None;
        let mut error_count: i64 = 0;

        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        let mut change_stream = collection
            .watch()
            .full_document(FullDocumentType::UpdateLookup)
            .full_document_before_change(FullDocumentBeforeChangeType::WhenAvailable)
            .start_at_operation_time(Timestamp {
                time: now,
                increment: 0,
            })
            .batch_size(100)
            .max_await_time(Duration::from_secs(30))
            .await?;

        log::info!("Started watching workload collection for changes...");

        // Listen for changes in the workload collection with improved error handling
        while let Some(change_event) = change_stream.next().await {
            match change_event {
                Ok(change_event) => {
                    let workload = match change_event.full_document {
                        Some(w) => w,
                        None => {
                            if change_event.operation_type
                                != mongodb::change_stream::event::OperationType::Delete
                            {
                                log::warn!("Received change event without full document");
                                error_count += 1;
                                continue;
                            }

                            log::warn!("Detected a manual deletion event. Our workload deletion logic is handled via metadata updates instead of a true deletion. Ignoring change...");
                            continue;
                        }
                    };

                    // Handle the workload change based on operation type
                    let api_result = match change_event.operation_type {
                        mongodb::change_stream::event::OperationType::Insert
                        | mongodb::change_stream::event::OperationType::Update => {
                            self.handle_workload_change_event(
                                workload,
                                change_event.full_document_before_change,
                            )
                            .await
                        }
                        _ => continue,
                    };

                    match api_result {
                        Ok(api_result) => {
                            // Publish response to hosts
                            self.publish_response_to_hosts(
                                jetstream.clone(),
                                service_subject.clone(),
                                response_subject_fn.clone(),
                                api_result,
                            )
                            .await;

                            // Reset error count after successful api result
                            error_count = 0;
                        }
                        Err(e) => {
                            log::error!(
                                "Error handling workload {:?}: {e:?}",
                                change_event.operation_type
                            );

                            // Increment error count after failed api result
                            error_count += 1;
                        }
                    }

                    // Store the resume token for potential recovery
                    last_resume_token = Some(change_event.id);
                }
                Err(e) => {
                    log::error!("Error in workload change stream: {}", e);
                    error_count += 1;

                    // Add backoff for mongodb reconnection (exponentially increases according to err count)
                    let backoff = Duration::from_secs(1 << error_count.min(5));
                    tokio::time::sleep(backoff).await;

                    // Attempt to reconnect using the last resume token if available
                    // and otherwise start from the current time
                    let mut watch = collection
                        .watch()
                        .full_document(FullDocumentType::UpdateLookup)
                        .batch_size(100)
                        .max_await_time(Duration::from_secs(30));

                    if let Some(token) = &last_resume_token {
                        log::info!(
                            "Attempting to reconnect to change stream resuming after token: {:?}",
                            token
                        );
                        watch = watch.resume_after(token.clone());
                    } else {
                        log::info!(
                            "No previous resume token found. Starting from the current time"
                        );
                        let now = SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as u32;
                        watch = watch.start_at_operation_time(Timestamp {
                            time: now,
                            increment: 0,
                        });
                    }

                    match watch.await {
                        Ok(new_stream) => {
                            change_stream = new_stream;
                            log::info!("Successfully reconnected to change stream");
                        }
                        Err(e) => {
                            log::error!("Failed to reconnect to change stream: {}", e);
                            return Err(e.into());
                        }
                    }
                }
            }
        }

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
        let db_name = std::env::var("HOLO_DATABASE_NAME").unwrap_or(DATABASE_NAME.to_string());
        Ok(MongoCollection::<T>::new(client, &db_name, collection_name).await?)
    }
}
