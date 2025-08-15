use anyhow::Result;
use bson::{doc, oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumDiscriminants, EnumString, FromRepr};
use utoipa::{openapi, PartialSchema, ToSchema};

use super::metadata::Metadata;
use super::{alias::PubKey, user_permissions::UserPermission};
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for user documents
pub const USER_COLLECTION_NAME: &str = "user";

/// Enumeration of possible user roles
/// Roles will apply a predefined set of permissions to the user automatically
#[derive(
    Debug,
    Clone,
    EnumString,
    Serialize,
    Deserialize,
    PartialEq,
    AsRefStr,
    EnumDiscriminants,
    FromRepr,
    Display,
)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    // WARNING: This role will give full access to the system
    Admin,
    // Role for customers to manage their own data
    User,
    // Role for developers or support team to have limited access over others data
    Support,
}

/// Information about a user's role (hoster or developer) in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserPubKey {
    pub pubkey: PubKey,
    pub is_developer: bool,
    pub is_hoster: bool,
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
    // a list of public keys
    pub public_keys: Vec<UserPubKey>,
}

impl IntoIndexes for User {
    /// Defines MongoDB indices for the User collection
    ///
    /// Creates indices for:
    /// - public_key.role
    /// - public_key.pubkey
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];
        Ok(indices)
    }
}

impl MutMetadata for User {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

impl PartialSchema for UserRole {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        let schema = openapi::schema::Object::builder()
            .schema_type(openapi::schema::SchemaType::Type(
                openapi::schema::Type::Object,
            ))
            .title(Some("Permission Action".to_string()))
            .examples(vec![
                UserRole::Admin.to_string(),
                UserRole::User.to_string(),
                UserRole::Support.to_string(),
            ])
            .build();

        openapi::RefOr::T(openapi::schema::Schema::Object(schema))
    }
}
impl ToSchema for UserRole {}
