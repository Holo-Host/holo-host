use anyhow::Result;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for hoster documents
pub const HOSTER_COLLECTION_NAME: &str = "hoster";

/// Hoster document schema representing a hoster in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Hoster {
    /// MongoDB ObjectId of the hoster document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub user_id: ObjectId,
    /// List of hosts managed by this hoster
    pub assigned_hosts: Vec<ObjectId>,
}

impl IntoIndexes for Hoster {
    /// No additional indices defined for Hoster collection
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl MutMetadata for Hoster {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}