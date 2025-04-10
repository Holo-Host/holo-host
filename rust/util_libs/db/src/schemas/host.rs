use bson::{doc, oid::ObjectId, Document};
use hpos_hal::inventory::HoloInventory;
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::mongodb::{MutMetadata, IntoIndexes};
use super::metadata::Metadata;

/// Collection name for host documents
pub const HOST_COLLECTION_NAME: &str = "host";

/// Host document schema representing a hosting device in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Host {
    /// MongoDB ObjectId of the host document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Unique identifier for the device
    pub device_id: String,
    /// Hardware inventory information
    pub inventory: HoloInventory,
    /// Average uptime as a percentage
    pub avg_uptime: f64,
    /// Average network speed in Mbps
    pub avg_network_speed: i64,
    /// Average latency in milliseconds
    pub avg_latency: i64,
    /// IP address of the host
    pub ip_address: Option<String>,
    /// Reference to the assigned hoster
    pub assigned_hoster: Option<ObjectId>,
    /// List of workloads running on this host
    pub assigned_workloads: Vec<ObjectId>,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            _id: None,
            metadata: Metadata::default(),
            device_id: Default::default(),
            inventory: HoloInventory::default(),
            avg_uptime: 100.00,     // Start with full 100% uptime
            avg_network_speed: 100, // Start at decent network speed (mbps)
            avg_latency: 100,       // Start at decent latency time
            assigned_workloads: vec![],
            assigned_hoster: None,
            ip_address: None,
        }
    }
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