use super::{
    types::{HostUpdateApiResult, HostUpdateRequest, HostUpdateResponseInfo, HostUpdateResult},
    HostUpdatesServiceApi, TAG_MAP_PREFIX_DESIGNATED_HOST,
};
use anyhow::Result;
use async_nats::Message;
use bson::{self, doc};
use db_utils::{
    mongodb::{api::MongoDbAPI, collection::MongoCollection},
    schemas::{
        host::{Host, HOST_COLLECTION_NAME},
        hoster::{Hoster, HOSTER_COLLECTION_NAME},
    },
};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nats_utils::types::ServiceError;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

#[derive(Clone, Debug)]
pub struct OrchestratorHostUpdatesApi {
    pub host_collection: MongoCollection<Host>,
    pub _hoster_collection: MongoCollection<Hoster>,
}

impl HostUpdatesServiceApi for OrchestratorHostUpdatesApi {}

impl OrchestratorHostUpdatesApi {
    pub async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            host_collection: Self::init_collection(client, HOST_COLLECTION_NAME).await?,
            _hoster_collection: Self::init_collection(client, HOSTER_COLLECTION_NAME).await?,
        })
    }

    // Invoked on `HOST.orchestrator.status`
    pub async fn handle_host_update(
        &self,
        msg: Arc<Message>,
    ) -> Result<HostUpdateApiResult, ServiceError> {
        let update_request = Self::convert_msg_to_type::<HostUpdateRequest>(msg)?;
        log::info!("Orchestrator::handle_host_update: msg={update_request:?}");

        // QUESTION: Do we need to retrieve or update data in the Host and/or Hoster collections?
        // If so, do that logic here...

        let channel = update_request.channel.clone();
        let device_id = update_request.device_id.clone();
        let info = HostUpdateResponseInfo {
            info: format!(
                "Requesting {:?} to preform update on nixos channel {}",
                update_request.device_id, update_request.channel
            ),
            // host_id: ObjectId,
            // hoster_id: ObjectId,
            request_info: update_request,
        };

        // Create tag map to fwd message onward to designated host
        let mut subject_tag_map = HashMap::new();
        subject_tag_map.insert(TAG_MAP_PREFIX_DESIGNATED_HOST.to_string(), device_id);

        log::info!(
            "Requesting updates on hosts. Channel={:#?}\nDeviceIds={:#?}",
            channel,
            subject_tag_map.values()
        );

        Ok(HostUpdateApiResult {
            result: HostUpdateResult::Success(info),
            maybe_response_tags: Some(subject_tag_map),
            maybe_headers: None,
        })
    }

    // Invoked on `HOST.orchestrator.status`
    pub async fn handle_host_update_response(
        &self,
        msg: Arc<Message>,
    ) -> Result<HostUpdateApiResult, ServiceError> {
        // Handle the response from holo-host-agent
        let update_result = Self::convert_msg_to_type::<HostUpdateResult>(msg)?;

        log::info!("Orchestrator::handle_host_update_response received: msg={update_result:?}");

        let host_update_status = match update_result.clone() {
            HostUpdateResult::Success(response_info) => {
                // manage any special logic for success case
                response_info
            }
            HostUpdateResult::Error(response_info) => {
                log::error!("Failed to update host: response_info={response_info:?}");
                response_info
            }
        };

        log::debug!(
            "Received host update response. Status={:?}",
            host_update_status
        );

        // Convert the status to be bson compatible (required for mongodb)
        let status_bson = bson::to_bson(&host_update_status).map_err(|e| {
            ServiceError::internal(
                e.to_string(),
                Some("Failed to serialize host update status".to_string()),
            )
        })?;

        // Update the host status in the db
        self.host_collection
            .update_one_within(
                doc! { "device_id": host_update_status.request_info.device_id },
                UpdateModifications::Document(doc! { "$set": { "status": status_bson } }),
                false,
            )
            .await?;

        Ok(HostUpdateApiResult {
            result: update_result,
            maybe_response_tags: None,
            maybe_headers: None,
        })
    }
}
