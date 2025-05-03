use serde::{Deserialize, Serialize};
use strum::{AsRefStr, EnumDiscriminants, EnumString, FromRepr};

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
