use super::{types::HostUpdateResult, HostUpdatesServiceApi};
use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use bson::{self, doc, oid::ObjectId, DateTime};
use db_utils::{
    mongodb::{
        api::MongoDbAPI,
        collection::MongoCollection,
        traits::{IntoIndexes, MutMetadata},
    },
    schemas::{
        self,
        host::{Host, HOST_COLLECTION_NAME},
        workload::{Workload, WORKLOAD_COLLECTION_NAME},
    },
};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, sync::Arc};
use types::HostUpdateResult;

// HOST.<machine_id>.update

// stream_subject.recipient.action
/// HOST.orchestrator.<>

#[derive(Clone, Debug)]
pub struct OrchestratorHostUpdatesApi {
    pub workload_collection: MongoCollection<Workload>,
    pub host_collection: MongoCollection<Host>,
}

impl HostUpdatesServiceApi for OrchestratorHostUpdatesApi {}

impl OrchestratorHostUpdatesApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, HOST_COLLECTION_NAME).await?,
        })
    }

    //
    async fn handle_host_update_response(
        &self,
        msg: Arc<Message>,
    ) -> Result<HostUpdateResult, ServiceError> {
        // handle the response from holo-host-agent
        let host_update_status = match Self::convert_msg_to_type::<HostUpdateResult>(msg)? {
            HostUpdateResult::Success(mut response_info) => {
                // do any logic for succss case
                response_info
            }
            HostUpdateResult::Error(mut response_info) => {
                log::error!("Failed to update host: response_info={response_info:?}");
                response_info
            }
        };

        log::debug!(
            "Received host update response. Status={:?}",
            host_update_status
        );

        // Update the workload status in the db
        let status_bson = bson::to_bson(&host_update_status).map_err(|e| {
            ServiceError::internal(
                e.to_string(),
                Some("Failed to serialize host update status".to_string()),
            )
        })?;

        // NB: unwrap is safe here because we check if it is set above
        self.host_collection
            .update_one_within(
                doc! { "_id": host_update_status.device_id },
                UpdateModifications::Document(doc! { "$set": { "status": status_bson } }),
                false,
            )
            .await?;

        Ok(HostUpdateResult {
            result: host_update_status,
            maybe_response_tags: None,
            maybe_headers: None,
        })
    }

    async fn handle_host_update(
        &self,
        update_request: HostUpdateRequest,
    ) -> Result<HostUpdateResult, ServiceError> {
        // log::info!("Orchestrator::handle_workload_assignment");

        // // Find minimum number of eligible hosts for the new workload
        // let min_eligible_hosts = self
        //     .get_min_random_hosts_for_workload(workload.clone())
        //     .await?;

        // log::debug!(
        //     "Eligible hosts for new workload. MongodDB Hosts={:?}",
        //     min_eligible_hosts
        // );

        // // Assign workload to hosts and create response
        // self.assign_workload_and_create_response(workload, min_eligible_hosts, target_state)
        //     .await
    }
}
