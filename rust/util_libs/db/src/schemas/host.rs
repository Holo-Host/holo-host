use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for host documents
pub const HOST_COLLECTION_NAME: &str = "host";

/// Host document schema representing a hosting device in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    /// MongoDB ObjectId of the host document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Owner of the host, typically a developer or organization
    pub owner: ObjectId,
    /// Unique identifier for the device
    pub device_id: String,
    /// Average uptime as a percentage
    pub avg_uptime: f64,
    /// Average network speed in Mbps
    pub avg_network_speed: i64,
    /// Average latency in milliseconds
    pub avg_latency: i64,
}

impl IntoIndexes for Host {
    /// Defines MongoDB indices for the Host collection
    ///
    /// Creates an index for:
    /// - device_id
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];
        //  Add Device ID Index
        let device_id_index_doc = doc! { "device_id": 1 };
        let device_id_index_opts = Some(
            IndexOptions::builder()
                .name(Some("device_id_index".to_string()))
                .build(),
        );
        indices.push((device_id_index_doc, device_id_index_opts));
        Ok(indices)
    }
}

impl MutMetadata for Host {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
