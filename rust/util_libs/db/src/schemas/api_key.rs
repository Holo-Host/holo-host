use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::{metadata::Metadata, user_permissions::UserPermission};
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for API key documents
pub const API_KEY_COLLECTION_NAME: &str = "api_key";

/// API key document schema representing an API key in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiKey {
    /// MongoDB ObjectId of the host document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,

    /// the user this api key belongs to
    pub owner: ObjectId,
    /// hashed api key
    pub api_key: String,
    /// the permissions this api key has
    pub permissions: Vec<UserPermission>,
    /// description of the API key (this is optional and set by the user)
    pub description: String,
    /// when the api key expires in unixtimestamp (seconds) (this is optional and set by the user)
    /// bson::DateTime::now().to_chrono().timestamp()
    pub expire_at: i64,
    /// prefix for the api key
    /// this will be used to locate the api key in the database
    pub prefix: Option<String>,
}

impl Default for ApiKey {
    fn default() -> Self {
        Self {
            _id: None,
            metadata: Metadata::default(),
            owner: ObjectId::new(),
            api_key: String::new(),
            permissions: vec![],
            description: "".to_string(),
            // default expire_at is 30 day
            expire_at: bson::DateTime::now().to_chrono().timestamp() + 60 + 60 * 24 * 30,
            prefix: None,
        }
    }
}

impl IntoIndexes for ApiKey {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // Create an index on the api_key field
        let api_key_index = doc! {
            "api_key": 1,
        };
        let api_key_index_options = IndexOptions::builder()
            .unique(true)
            .name("api_key_index".to_string())
            .build();
        indices.push((api_key_index, Some(api_key_index_options)));

        // create an index on the owner field
        let owner_index = doc! {
            "owner": 1,
        };
        let owner_index_options = IndexOptions::builder()
            .name("owner_index".to_string())
            .build();
        indices.push((owner_index, Some(owner_index_options)));

        Ok(indices)
    }
}

impl MutMetadata for ApiKey {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
