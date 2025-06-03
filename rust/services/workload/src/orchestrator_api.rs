/*
Current Endpoints & Managed Subjects:
    - `add_workload`: handles the "WORKLOAD.add" subject
    - `update_workload`: handles the "WORKLOAD.update" subject
    - `delete_workload`: handles the "WORKLOAD.delete" subject
    - `handle_db_insertion`: handles the "WORKLOAD.insert" subject // published by mongo<>nats connector
    - `handle_workload_change_event`: handles the "WORKLOAD.modify" subject // published by mongo<>nats connector
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
        developer::{Developer, DEVELOPER_COLLECTION_NAME},
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
    options::{FullDocumentType, UpdateModifications},
    Client as MongoDBClient,
};
use nats_utils::types::CreateResponse;
use nats_utils::types::CreateTag;
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
    pub developer_collection: MongoCollection<Developer>,
}

impl WorkloadServiceApi for OrchestratorWorkloadApi {}

impl OrchestratorWorkloadApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, HOST_COLLECTION_NAME).await?,
            user_collection: Self::init_collection(client, USER_COLLECTION_NAME).await?,
            developer_collection: Self::init_collection(client, DEVELOPER_COLLECTION_NAME).await?,
        })
    }

    pub async fn add_workload(&self, msg: Arc<Message>) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.add'");
        self.process_request(
            msg,
            WorkloadState::Reported,
            WorkloadState::Error,
            |mut workload: Workload| async move {
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
        log::debug!("Incoming message for {}", &msg.subject);

        self.process_request(
            msg,
            WorkloadState::Updating,
            WorkloadState::Error,
            |mut workload: Workload| async move {
                let status = WorkloadStatus {
                    id: workload._id,
                    desired: WorkloadState::Updated,
                    actual: WorkloadState::Updating,
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
            |workload: Workload| async move {
                let workload_id = if let Some(workload_id) = workload._id {
                    workload_id
                } else {
                    return Ok(WorkloadApiResult {
                        result: WorkloadResult {
                            status: WorkloadStatus {
                                ..workload.status.clone()
                            },
                            workload: Some(workload),
                        },
                        maybe_response_tags: None,
                    });
                };

                let status = WorkloadStatus {
                    id: Some(workload_id),
                    desired: WorkloadState::Removed,
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
                        doc! { "_id":  workload_id },
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
                false,
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

    pub async fn manage_workload_on_host(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.insert'");
        let workload = Self::convert_msg_to_type::<Workload>(msg)?;
        match workload.status.desired {
            WorkloadState::Removed | WorkloadState::Deleted | WorkloadState::Uninstalled => {
                self.handle_workload_change_event(workload).await
            }
            WorkloadState::Running => match workload.status.actual {
                WorkloadState::Assigned | WorkloadState::Updating => {
                    self.handle_workload_change_event(workload).await
                }
                _ => Err(ServiceError::internal(
                    "Received invalid actual state for host update.".to_string(),
                    Some("WorkloadApiError::manage_workload_on_host".to_string()),
                )),
            },
            _ => Err(ServiceError::internal(
                "Received invalid desired state for host update.".to_string(),
                Some("WorkloadApiError::manage_workload_on_host".to_string()),
            )),
        }
    }

    async fn handle_workload_change_event(
        &self,
        workload: Workload,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let workload_id = workload._id.ok_or_else(|| {
            ServiceError::internal(
                format!(
                    "No `_id` found for workload. Unable to proceed assigning a host. Workload={:?}",
                    workload
                ),
                Some("Missing workload ID".to_string()),
            )
        })?;

        // Match on state and handle each case
        match workload.status.actual {
            WorkloadState::Reported => {
                log::debug!("Detected new workload to assign. Workload={:#?}", workload);
                self.handle_workload_assignment(workload, workload_id, WorkloadState::Assigned)
                    .await
            }
            WorkloadState::Updating => {
                log::trace!(
                    "Detected workload updated to handle. Workload={:#?}",
                    workload
                );
                self.handle_workload_update(workload, workload_id).await
            }
            WorkloadState::Deleted => {
                log::trace!(
                    "Detected workload deletion to handle. Workload={:#?}",
                    workload
                );
                self.handle_workload_deletion(workload, workload_id).await
            }
            _ => Ok(WorkloadApiResult {
                // Catches all other cases wherein a record in the workload collection was modified
                // with a state other than "Reported", "Updating", or "Deleted".
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
            }),
        }
    }

    async fn handle_workload_assignment(
        &self,
        workload: Workload,
        workload_id: ObjectId,
        target_state: WorkloadState,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Find eligible hosts for the new workload
        let eligible_hosts = self.find_hosts_for_workload(workload.clone()).await?;

        log::debug!(
            "Eligible hosts for new workload. MongodDB Hosts={:?}",
            eligible_hosts
        );

        // Assign workload to hosts and create response
        self.assign_workload_and_create_response(
            workload,
            workload_id,
            eligible_hosts,
            target_state,
        )
        .await
    }

    async fn handle_workload_update(
        &self,
        workload: Workload,
        workload_id: ObjectId,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Fetch current hosts and remove workload from them
        self.remove_workload_from_hosts(workload_id).await?;
        self.handle_workload_assignment(workload, workload_id, WorkloadState::Updated)
            .await
    }

    // TODO: Only delete/unpair hosts from workload collection upon receiving uninsalled confirmation back frlm hos
    async fn handle_workload_deletion(
        &self,
        workload: Workload,
        workload_id: ObjectId,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Fetch current hosts and remove workload from them
        let hosts = self.fetch_hosts_assigned_to_workload(workload_id).await?;
        self.remove_workload_from_hosts(workload_id).await?;

        let new_status = WorkloadStatus {
            id: Some(workload_id),
            desired: WorkloadState::Uninstalled,
            actual: WorkloadState::Deleted,
            payload: Default::default(),
        };

        // Remove hosts from the workload and update status
        self.assign_hosts_to_workload(workload_id, vec![], new_status.clone())
            .await?;
        log::info!(
            "Workload update in DB successful. Fwding update to assigned hosts. workload_id={} Hosts={:?}",
            workload_id,
            hosts
        );
        // Create tag map for response
        let mut tag_map = HashMap::new();
        for (index, host) in hosts.iter().enumerate() {
            let host_id = host._id.ok_or_else(|| {
                ServiceError::internal(
                    "Host missing ID".to_string(),
                    Some("Database integrity error".to_string()),
                )
            })?;
            tag_map.insert(
                format!("{TAG_MAP_PREFIX_ASSIGNED_HOST}{}", index),
                host_id.to_hex(),
            );
        }

        log::trace!("Subject tag map: {tag_map:?}");

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status: WorkloadStatus {
                    id: Some(workload_id),
                    desired: WorkloadState::Uninstalled,
                    actual: WorkloadState::Removed,
                    payload: Default::default(),
                },
                workload: Some(workload),
            },
            maybe_response_tags: Some(tag_map),
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
    async fn find_hosts_for_workload(
        &self,
        workload: Workload,
    ) -> Result<Vec<HostIdJSON>, ServiceError> {
        let needed_host_count = workload.min_hosts;

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
        workload_id: ObjectId,
        assigned_host_ids: Vec<ObjectId>,
        new_status: WorkloadStatus,
    ) -> Result<(), ServiceError> {
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

    async fn assign_workload_to_hosts(
        &self,
        workload_id: ObjectId,
        eligible_host_ids: Vec<ObjectId>,
        needed_host_count: i32,
    ) -> Result<Vec<ObjectId>, ServiceError> {
        // NB: This will attempt to assign the hosts up to 5 times.. then exit loop with warning message
        let mut unassigned_host_ids: Vec<ObjectId> = eligible_host_ids.clone();
        let mut exit_flag = 0;

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
                    "assigned_workloads": { "$size": 0 }
                })
                .await?;

            unassigned_host_ids = unassigned_hosts.into_iter().filter_map(|h| h._id).collect();
            exit_flag += 1;
        }
    }

    async fn assign_workload_and_create_response(
        &self,
        workload: Workload,
        workload_id: ObjectId,
        eligible_hosts: Vec<HostIdJSON>,
        target_state: WorkloadState,
    ) -> Result<WorkloadApiResult, ServiceError> {
        // Assign workload to hosts
        let eligible_host_ids: Vec<ObjectId> = eligible_hosts.iter().map(|h| h._id).collect();
        let assigned_host_ids = self
            .assign_workload_to_hosts(workload_id, eligible_host_ids, workload.min_hosts)
            .await?;

        // Update workload status and assigned hosts
        let new_status = WorkloadStatus {
            id: None,
            desired: WorkloadState::Running,
            actual: target_state,
            payload: Default::default(),
        };
        self.assign_hosts_to_workload(workload_id, assigned_host_ids.clone(), new_status.clone())
            .await?;

        // Create tag map for response
        let mut tag_map = HashMap::new();
        for (index, host_id) in assigned_host_ids.iter().enumerate() {
            let assigned_host = eligible_hosts
                .iter()
                .find(|h| h._id == *host_id)
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

        if !tag_map.is_empty() {
            log::info!(
                "Assigned workload to hosts. Workload={:#?}\nDeviceIds={:#?}",
                workload_id,
                tag_map.values()
            );
        }

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status: WorkloadStatus {
                    id: Some(workload_id),
                    ..new_status
                },
                workload: Some(workload),
            },
            maybe_response_tags: Some(tag_map),
        })
    }

    pub async fn publish_response_to_hosts(
        &self,
        jetstream: Context,
        service_subject: String,
        response_subject_fn: ResponseSubjectsGenerator,
        workload_api_result: WorkloadApiResult,
    ) {
        let response_bytes = workload_api_result.get_response();
        let response_subjects = response_subject_fn(workload_api_result.get_tags());
        for response_subject in response_subjects.iter() {
            let subject = format!("{}.{}", service_subject, response_subject);
            log::debug!("publishing a response on {subject}");
            if let Err(err) = jetstream
                .publish(subject.clone(), response_bytes.clone())
                .await
            {
                log::error!(
                    "WORKLOAD_API_LOG::Failed to publish new message to host: subj='{}', service={}, err={:?}",
                    subject,
                    "publish_response_to_hosts",
                    err
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
                            log::warn!("Received change event without full document");
                            error_count += 1;
                            continue;
                        }
                    };

                    // Handle the workload change based on operation type
                    let api_result = match change_event.operation_type {
                        mongodb::change_stream::event::OperationType::Insert => {
                            self.handle_workload_change_event(workload).await
                        }
                        mongodb::change_stream::event::OperationType::Update => {
                            self.handle_workload_change_event(workload).await
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
                        }
                        Err(e) => {
                            log::error!(
                                "Error handling workload {:?}: {e:?}",
                                change_event.operation_type
                            );
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
                    // and otherwise start from current time
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
                        log::info!("No previous resume token found, starting from current time");
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
