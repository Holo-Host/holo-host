use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for developer documents
pub const USER_INFO_COLLECTION_NAME: &str = "user_info";

/// Additional user information schema
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserInfo {
    /// MongoDB ObjectId of the user info document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub user_id: ObjectId,
    /// User's email address
    pub email: String,
    /// User's given names
    pub given_names: String,
    /// User's family name
    pub family_name: String,
}

impl IntoIndexes for UserInfo {
    /// Defines MongoDB indices for the UserInfo collection
    ///
    /// Creates an index for:
    /// - email address
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];
        // add email index
        let email_index_doc = doc! { "email": 1 };
        let email_index_opts = Some(
            IndexOptions::builder()
                .name(Some("email_index".to_string()))
                .build(),
        );
        indices.push((email_index_doc, email_index_opts));
        Ok(indices)
    }
}

impl MutMetadata for UserInfo {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
