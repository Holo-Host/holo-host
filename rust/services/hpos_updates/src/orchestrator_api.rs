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
        host::{Host, HostStatus, HOST_COLLECTION_NAME},
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

        let (host_update_status, channel, device_id) = match update_result.clone() {
            HostUpdateResult::Success(response) => {
                // TODO: manage any special logic for success case
                (
                    HostStatus::Active(response.info),
                    response.request_info.channel,
                    response.request_info.device_id,
                )
            }
            HostUpdateResult::Error(response) => {
                log::error!("Failed to update host: response_info={:?}", response.info);
                (
                    HostStatus::Error(response.info),
                    response.request_info.channel,
                    response.request_info.device_id,
                )
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
                doc! { "device_id": device_id },
                UpdateModifications::Document(doc! { "$set":
                    { "status": status_bson, "channel": channel
                } }),
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
// Tests:
// TODO: Separate out into diff dir (in line with repo pattern)
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use db_utils::mongodb::api::MongoDbAPI;
    use db_utils::schemas::DATABASE_NAME;
    use mock_utils::host::create_test_host;
    use mock_utils::mongodb_runner::MongodRunner;
    use mock_utils::nats_message::NatsMessage;
    use std::sync::Arc;
    // use db_utils::schemas::host::Host;

    #[tokio::test]
    async fn test_handle_host_update() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");
        let api = OrchestratorHostUpdatesApi::new(&db_client).await?;

        // Insert a test host and hoster (minimal, not strictly required for current logic)
        let host = create_test_host("test_device_id", None, None, None, None, None);
        let _host_id = api.host_collection.insert_one_into(host).await?;

        let update_request = HostUpdateRequest {
            channel: "nixos-unstable".to_string(),
            device_id: "test_device_id".to_string(),
        };
        let msg_payload = serde_json::to_vec(&update_request).unwrap();
        let msg =
            Arc::new(NatsMessage::new("HOST.orchestrator.update", msg_payload).into_message());
        let r = api.handle_host_update(msg).await?;

        if let HostUpdateResult::Success(info) = r.result {
            assert_eq!(info.request_info.device_id, "test_device_id");
            assert_eq!(info.request_info.channel, "nixos-unstable");
            assert!(r.maybe_response_tags.is_some());
            let tags = r.maybe_response_tags.unwrap();
            assert!(tags.contains_key(TAG_MAP_PREFIX_DESIGNATED_HOST));
            assert_eq!(tags[TAG_MAP_PREFIX_DESIGNATED_HOST], "test_device_id");
        } else {
            panic!("Expected HostUpdateResult::Success, got something else");
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_host_update_response() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");
        let api = OrchestratorHostUpdatesApi::new(&db_client).await?;

        // Insert a test host
        let mut host = create_test_host("test_device_id", None, None, None, None, None);
        host.device_id = "test_device_id".to_string();
        let _host_id = api.host_collection.insert_one_into(host).await?;

        let update_request = HostUpdateRequest {
            channel: "nixos-unstable".to_string(),
            device_id: "test_device_id".to_string(),
        };
        let response_info = HostUpdateResponseInfo {
            info: "Update performed successfully".to_string(),
            request_info: update_request.clone(),
        };
        let update_result = HostUpdateResult::Success(response_info.clone());
        let msg_payload = serde_json::to_vec(&update_result).unwrap();
        let msg =
            Arc::new(NatsMessage::new("HOST.orchestrator.status", msg_payload).into_message());
        let r = api.handle_host_update_response(msg).await?;

        if let HostUpdateResult::Success(info) = r.result {
            assert_eq!(info.request_info.device_id, "test_device_id");
            assert_eq!(info.request_info.channel, "nixos-unstable");
        } else {
            panic!("Expected HostUpdateResult::Success, got something else");
        }

        // Check that the host status was updated in the DB
        let raw_doc = db_client
            .database(DATABASE_NAME)
            .collection::<bson::Document>("host")
            .find_one(bson::doc! { "device_id": "test_device_id" })
            .await?
            .expect("Host should exist");

        println!("raw_doc >>>>>>>>>>> {:#?}", raw_doc);
        assert!(raw_doc.get("status").is_some());
        assert!(raw_doc.get("channel").is_some());
        assert_eq!(
            raw_doc.get("channel").unwrap().as_str().unwrap(),
            "nixos-unstable"
        );
        assert!(raw_doc.get("status").is_some());
        let status_bson = raw_doc.get("status").unwrap();
        let status: HostStatus = bson::from_bson(status_bson.clone()).unwrap();
        assert_eq!(
            status,
            HostStatus::Active("Update performed successfully".to_string())
        );

        Ok(())
    }
}
