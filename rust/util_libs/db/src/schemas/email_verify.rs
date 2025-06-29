use anyhow::Result;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::{derive_with_metadata, derive_with_mongo_id, mongodb::traits::IntoIndexes};

/// Collection name for hoster documents
pub const EMAIL_VERIFY_COLLECTION_NAME: &str = "email_verify";

/// Hoster document schema representing a hoster in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EmailVerify {
    /// MongoDB ObjectId of the hoster document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// the email address to verify
    pub email: String,
    /// the code required for verification
    pub code: String,
}

impl IntoIndexes for EmailVerify {
    /// Defines MongoDB indices for the Host collection
    ///
    /// Creates an index for:
    /// - email
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];
        //  Add Device ID Index
        let email_index_doc = bson::doc! { "email": 1 };
        let email_index_opts = Some(
            IndexOptions::builder()
                .name(Some("email_index".to_string()))
                .build(),
        );
        indices.push((email_index_doc, email_index_opts));
        Ok(indices)
    }
}

derive_with_metadata!(EmailVerify);
derive_with_mongo_id!(EmailVerify);
