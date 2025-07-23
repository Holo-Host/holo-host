mod orchestrator_api;
mod types;

use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use async_trait::async_trait;
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

// api_subject.recipient.action
/// WORKLOAD.orchestrator.status
/// WORKLOAD.<device_id>.update
/// 
/// NB: Blob store subject:
/// BLOB_STORE.<object_id>.fetch
/// 
/// HOST.orchestrator.update
/// HOST.orchestrator.status
/// HOST.<device_id>.update

pub const HOST_UPDATES_SRV_NAME: &str = "HOST";
pub const HOST_UPDATES_SRV_SUBJ: &str = "HOST";
pub const HOST_UPDATES_SRV_VERSION: &str = "0.0.1";
pub const HOST_UPDATES_SRV_DESC: &str =
    "This service handles the on-command holo-host-agent updates.";

// Service Endpoint Names:
pub const HOST_UPDATES_SUBJECT: &str = "update";

// Tag to identify host id
pub const TAG_MAP_PREFIX_ASSIGNED_HOST: &str = "assigned_host";
// Tag to identify the orchestrator prefix
pub const ORCHESTRATOR_SUBJECT_PREFIX: &str = "orchestrator";

#[derive(Clone, Debug)]
pub struct HostUpdatesServiceApi {
    pub workload_collection: MongoCollection<Workload>,
    pub host_collection: MongoCollection<Host>,
}

#[async_trait]
pub trait WorkloadServiceApi
where
    Self: std::fmt::Debug + 'static,
{
    async fn new(client: &MongoDBClient) -> Result<Self> {
        Ok(Self {
            workload_collection: Self::init_collection(client, WORKLOAD_COLLECTION_NAME).await?,
            host_collection: Self::init_collection(client, HOST_COLLECTION_NAME).await?,
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
}
