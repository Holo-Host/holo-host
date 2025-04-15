use anyhow::Result;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for developer documents
pub const DEVELOPER_COLLECTION_NAME: &str = "developer";

/// Developer document schema representing a developer in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Developer {
    /// MongoDB ObjectId of the developer document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub user_id: ObjectId,
    /// List of workloads created by this developer
    pub active_workloads: Vec<ObjectId>,
}

impl IntoIndexes for Developer {
    /// No additional indices defined for Developer collection
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl MutMetadata for Developer {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
