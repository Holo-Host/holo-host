use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::metadata::Metadata;
use super::user_permissions::UserPermission;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for user documents
pub const USER_COLLECTION_NAME: &str = "user";

/// Enumeration of possible user roles
/// Roles will apply a predefined set of permissions to the user automatically
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    // WARNING: This role will give full access to the system
    Admin,
    // Role for customers to manage their own data
    User,
    // Role for developers or support team to have limited access over others data
    Support,
}

/// User document schema representing a user in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    /// MongoDB ObjectId of the user document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// List of permissions the user has been granted
    pub permissions: Vec<UserPermission>,
    // A list of roles attached to the user
    pub roles: Vec<UserRole>,
    // this is used to invalidate all refresh tokens by incrementing the version by 1
    pub refresh_token_version: i32,
}

impl IntoIndexes for User {
    /// Defines MongoDB indices for the User collection
    ///
    /// Creates indices for:
    /// - public_key.role
    /// - public_key.pubkey
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

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
