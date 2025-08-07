use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::{derive_with_metadata, derive_with_mongo_id, mongodb::traits::IntoIndexes};

/// Collection name for developer documents
pub const USER_PASSWORD_COLLECTION_NAME: &str = "user_password";

/// Schema for User Passwords
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserPassword {
    /// MongoDB ObjectId of the user info document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub owner: ObjectId,
    /// hashed password
    pub password_hash: String,
}

derive_with_mongo_id!(UserPassword);
derive_with_metadata!(UserPassword);

impl IntoIndexes for UserPassword {
    /// Defines MongoDB indices for the UserInfo collection
    ///
    /// Creates an index for:
    /// - owner
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];
        // add owner index
        let owner_index_doc = doc! { "owner": 1 };
        let owner_index_opts = Some(
            IndexOptions::builder()
                .name(Some("owner_index".to_string()))
                .build(),
        );
        indices.push((owner_index_doc, owner_index_opts));
        Ok(indices)
    }
}
