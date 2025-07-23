pub mod host_api;
pub mod orchestrator_api;
pub mod types;

use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use async_trait::async_trait;
use db_utils::mongodb::{
    collection::MongoCollection,
    traits::{IntoIndexes, MutMetadata},
};
use db_utils::schemas;
use mongodb::Client as MongoDBClient;
use nats_utils::types::ServiceError;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, sync::Arc};

// api_subject.recipient.action
// WORKLOAD.orchestrator.status
// WORKLOAD.<device_id>.update
//
// NB: Blob store subject:
// BLOB_STORE.<object_id>.fetch
//
// HOST.orchestrator.update
// HOST.orchestrator.status
// HOST.<device_id>.update

pub const HOST_UPDATES_SRV_NAME: &str = "HOST";
pub const HOST_UPDATES_SRV_SUBJ: &str = "HOST";
pub const HOST_UPDATES_SRV_VERSION: &str = "0.0.1";
pub const HOST_UPDATES_SRV_DESC: &str =
    "This service handles the on-command holo-host-agent updates.";

// Tag to identify host id
pub const TAG_MAP_PREFIX_DESIGNATED_HOST: &str = "designated_host";

// Tag to identify the orchestrator prefix
pub const ORCHESTRATOR_SUBJECT_PREFIX: &str = "orchestrator";

#[async_trait]
pub trait HostUpdatesServiceApi
where
    Self: std::fmt::Debug + 'static,
{
    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        serde_json::from_slice::<T>(&msg.payload).map_err(|e| {
            ServiceError::request(
                format!("Failed to deserialize payload: {}", e),
                Some(ErrorCode::BAD_REQUEST),
            )
        })
    }

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
        let db_name =
            std::env::var("HOLO_DATABASE_NAME").unwrap_or(schemas::DATABASE_NAME.to_string());
        Ok(MongoCollection::<T>::new(client, &db_name, collection_name).await?)
    }
}
