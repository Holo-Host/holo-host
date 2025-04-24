use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumDiscriminants, EnumString, FromRepr};

use super::alias::PubKey;
use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for user documents
pub const USER_COLLECTION_NAME: &str = "user";

/// Enumeration of possible user roles
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumDiscriminants, FromRepr,
)]
#[strum_discriminants(
    derive(EnumString, Serialize, Deserialize),
    repr(usize),
    strum(serialize_all = "snake_case")
)]
pub enum PublicKeyRole {
    /// Role for hosters
    Hoster,
    /// Role for developers
    Developer,
}

/// Key pair of public key and it's role
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PublicKeyWithRole {
    // Role of the public key
    pub role: PublicKeyRole,
    /// Public key associated with the role
    pub pubkey: PubKey,
}

/// Represents the type of permission the user has for each resources
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumDiscriminants, FromRepr,
)]
#[strum_discriminants(
    derive(EnumString, Serialize, Deserialize),
    repr(usize),
    strum(serialize_all = "snake_case")
)]
pub enum PermissionType {
    Read,
    Create,
    Update,
    Delete,
}

/// Represents what the resource is and what type of permission the user has
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserPermission {
    pub resource: String,
    pub permission_type: PermissionType,
}

/// Enumeration of possible user roles
/// Roles will apply a predefined set of permissions to the user automatically
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumDiscriminants, FromRepr,
)]
#[strum_discriminants(
    derive(EnumString, Serialize, Deserialize),
    repr(usize),
    strum(serialize_all = "snake_case")
)]
pub enum UserRole {
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
    /// List of permissions the user has been granted
    pub permissions: Vec<UserPermission>,
    // A list of roles attached to the user
    pub roles: Vec<UserRole>,
    // contains a list of pairs of public keys and their roles
    pub public_key: Vec<PublicKeyWithRole>,
}

impl IntoIndexes for User {
    /// Defines MongoDB indices for the User collection
    ///
    /// Creates indices for:
    /// - public_key.role
    /// - public_key.pubkey
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        let public_key_role_doc = doc! { "public_key.role": 1 };
        let public_key_role_opts = Some(
            IndexOptions::builder()
                .name(Some("public_key_role_index".to_string()))
                .build(),
        );
        indices.push((public_key_role_doc, public_key_role_opts));

        let public_key_role_pubkey_doc = doc! { "public_key.pubkey": 1 };
        let public_key_role_pubkey_opts = Some(
            IndexOptions::builder()
                .name(Some("public_key.pubkey".to_string()))
                .build(),
        );
        indices.push((public_key_role_pubkey_doc, public_key_role_pubkey_opts));

        Ok(indices)
    }
}

impl MutMetadata for User {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
