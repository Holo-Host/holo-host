use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::mongodb::traits::{MutMetadata, IntoIndexes};
use super::metadata::Metadata;
use super::alias::PubKey;

/// Collection name for user documents
pub const USER_COLLECTION_NAME: &str = "user";

/// Information about a user's role (hoster or developer) in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoleInfo {
    /// MongoDB ObjectId reference to the role collection (hoster/developer)
    pub collection_id: ObjectId,
    /// Public key associated with the role
    pub pubkey: PubKey,
}


/// Enumeration of possible user permission levels
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserPermission {
    /// Administrator level permissions
    Admin,
}


/// User document schema representing a user in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    /// MongoDB ObjectId of the user document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// User's jurisdiction
    pub jurisdiction: String,
    /// List of user permissions
    pub permissions: Vec<UserPermission>,
    /// Reference to additional user information
    pub user_info_id: Option<ObjectId>,
    /// Developer role information if user is a developer
    pub developer: Option<RoleInfo>,
    /// Hoster role information if user is a hoster
    pub hoster: Option<RoleInfo>,
}

impl IntoIndexes for User {
    /// Defines MongoDB indices for the User collection
    ///
    /// Creates indices for:
    /// - user_info_id
    /// - developer role
    /// - hoster role
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // add user_info_id index
        let user_info_id_index_doc = doc! { "user_info_id": 1 };
        let user_info_id_index_opts = Some(
            IndexOptions::builder()
                .name(Some("user_info_id_index".to_string()))
                .build(),
        );
        indices.push((user_info_id_index_doc, user_info_id_index_opts));

        // add developer index
        let developer_index_doc = doc! { "developer": 1 };
        let developer_index_opts = Some(
            IndexOptions::builder()
                .name(Some("developer_index".to_string()))
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        // add host index
        let host_index_doc = doc! { "hoster": 1 };
        let host_index_opts = Some(
            IndexOptions::builder()
                .name(Some("hoster_index".to_string()))
                .build(),
        );
        indices.push((host_index_doc, host_index_opts));

        Ok(indices)
    }
}

impl MutMetadata for User {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
