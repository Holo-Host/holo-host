use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for host documents
pub const LOG_COLLECTION_NAME: &str = "api_logs";

/// Host document schema representing a hosting device in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ApiLog {
    /// MongoDB ObjectId of the host document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,

    /// Unique identifier for the API request
    pub request_id: String,
    /// the endpoint that was called
    pub path: String,
    /// the HTTP method used (GET, POST, etc.)
    pub method: String,
    /// the IP address of the client making the request
    pub ip: String,
    /// the user agent string of the client making the request
    pub user_agent: String,
    /// the authorization header value
    pub authorization: String,
    /// the user ID of the client making the request
    pub user_id: String,
    /// the timestamp of the request
    pub response_status: i32,
}

impl Default for ApiLog {
    fn default() -> Self {
        Self {
            _id: None,
            metadata: Metadata::default(),
            request_id: bson::uuid::Uuid::new().to_string(),
            path: String::new(),
            method: String::new(),
            ip: String::new(),
            user_agent: String::new(),
            authorization: String::new(),
            user_id: String::new(),
            response_status: 0,
        }
    }
}

impl IntoIndexes for ApiLog {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let indices = vec![];
        Ok(indices)
    }
}

impl MutMetadata for ApiLog {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
