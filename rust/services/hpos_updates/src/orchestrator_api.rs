use super::{
    types::{HostUpdateApiRequest, HostUpdateInfo, HostUpdateRequest, HostUpdateState},
    HposUpdatesServiceApi, TAG_MAP_PREFIX_DESIGNATED_HOST,
};
use anyhow::Result;
use async_nats::Message;
use bson::{self, doc};
use db_utils::{
    mongodb::{api::MongoDbAPI, collection::MongoCollection},
    schemas::{
        host::{Host, HostStatus, HOST_COLLECTION_NAME},
        hoster::{Hoster, HOSTER_COLLECTION_NAME},
    },
};
use mongodb::{options::UpdateModifications, Client as MongoDBClient};
use nats_utils::types::ServiceError;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

#[derive(Clone, Debug)]
pub struct OrchestratorHposUpdatesApi {
    pub host_collection: MongoCollection<Host>,
    pub _hoster_collection: MongoCollection<Hoster>,
}

impl HposUpdatesServiceApi for OrchestratorHposUpdatesApi {}

impl OrchestratorHposUpdatesApi {
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
    ) -> Result<HostUpdateApiRequest, ServiceError> {
        let update_request: HostUpdateRequest =
            Self::convert_msg_to_type::<HostUpdateRequest>(msg)?;
        log::info!("Orchestrator::handle_host_update: msg={update_request:?}");

        // QUESTION: Do we need to retrieve or update data in the Host and/or Hoster collections?
        // If so, do that logic here...

        let channel = update_request.channel.clone();
        let device_id = update_request.device_id.clone();
        let info = HostUpdateInfo {
            context: Some(format!(
                "Requesting {:?} to preform update on nixos channel {}",
                update_request.device_id, update_request.channel
            )),
            request_info: update_request,
            state: HostUpdateState::Pending,
        };

        // Create tag map to fwd message onward to designated host
        let mut subject_tag_map = HashMap::new();
        subject_tag_map.insert(TAG_MAP_PREFIX_DESIGNATED_HOST.to_string(), device_id);

        log::info!(
            "Requesting updates on hosts. Channel={:#?}\nDeviceIds={:#?}",
            channel,
            subject_tag_map.values()
        );

        Ok(HostUpdateApiRequest {
            info,
            maybe_response_tags: Some(subject_tag_map),
            maybe_headers: None,
        })
    }

    // Invoked on `HPOS.orchestrator.status`
    // Automatically published to via the callback in the holo-host-agent hpos-update service
    pub async fn handle_host_update_response(
        &self,
        msg: Arc<Message>,
    ) -> Result<HostUpdateApiRequest, ServiceError> {
        let host_update_info = Self::convert_msg_to_type::<HostUpdateInfo>(msg)?;
        log::info!("Orchestrator::handle_host_update_response received: msg={host_update_info:?}");

        let host_update_status = match host_update_info.state {
            HostUpdateState::Completed => {
                // TODO: Add any special logic for success case
                HostStatus::Active(
                    (host_update_info
                        .context
                        .as_ref()
                        .unwrap_or(&"Running".to_string()))
                    .to_string(),
                )
            }
            HostUpdateState::Failed => {
                log::error!(
                    "Failed to update host. Reporting error to db. response={:?}",
                    host_update_info
                );
                HostStatus::Error(
                    (host_update_info
                        .context
                        .as_ref()
                        .unwrap_or(&"Error".to_string()))
                    .to_string(),
                )
            }
            HostUpdateState::Pending => {
                log::warn!(
                    "Received unexpected state in host update response. Reporting error to db. response={:?}",
                    host_update_info
                );
                HostStatus::Error(format!(
                    "Host Nixos update status reported as still Pending. device_id={}, channel={}",
                    host_update_info.request_info.device_id, host_update_info.request_info.channel
                ))
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

        let device_id = host_update_info.request_info.device_id.clone();
        let channel = host_update_info.request_info.channel.clone();

        // Update the host status in the db
        self.host_collection
            .update_one_within(
                doc! { "device_id": device_id },
                UpdateModifications::Document(doc! { "$set":
                    { "status": status_bson, "channel": channel
                } }),
                false,
            )
            .await?;

        Ok(HostUpdateApiRequest {
            info: host_update_info,
            maybe_response_tags: None,
            maybe_headers: None,
        })
    }
}
