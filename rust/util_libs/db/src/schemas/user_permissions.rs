use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumDiscriminants, EnumString, FromRepr};
use utoipa::ToSchema;

/// Represents the type of permission the user has for each resources
#[derive(
    Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, AsRefStr, EnumDiscriminants, FromRepr,
)]
#[strum_discriminants(
    derive(EnumString, Serialize, Deserialize),
    repr(usize),
    strum(serialize_all = "snake_case")
)]
pub enum PermissionAction {
    All,
    Read,
    Create,
    Update,
    Delete,
}

/// Represents what the resource is and what type of permission the user has
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub struct UserPermission {
    /// The type of resource the user has access to (user, api_key, workload, etc.)
    #[schema(example = "all")]
    pub resource: String,
    /// What actions can the user perform on the resource
    #[schema(example = PermissionAction::All)]
    pub action: PermissionAction,
    /// Who owns the resource, This refers to the user id.
    /// If this is set to "self", it means the user has access to their own resources
    #[schema(example = "self")]
    pub owner: String,
    /// If true, then the owner field is ignored, mainly used by admin
    #[schema(example = false)]
    pub all_owners: bool,
}
