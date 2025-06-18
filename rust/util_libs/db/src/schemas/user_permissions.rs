use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumDiscriminants, EnumString, FromRepr};
use utoipa::{openapi, PartialSchema, ToSchema};

/// Represents the type of permission the user has for each resources
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    EnumString,
    PartialEq,
    AsRefStr,
    EnumDiscriminants,
    FromRepr,
    Display,
)]
#[serde(rename_all = "snake_case")]
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
    /// If set to "all" it means the user has access to resources owned by all users
    #[schema(example = "self")]
    pub owner: String,
}

impl PartialSchema for PermissionAction {
    fn schema() -> openapi::RefOr<openapi::schema::Schema> {
        let schema = openapi::schema::Object::builder()
            .schema_type(openapi::schema::SchemaType::Type(
                openapi::schema::Type::Object,
            ))
            .title(Some("Permission Action".to_string()))
            .examples(vec![
                PermissionAction::All.to_string(),
                PermissionAction::Read.to_string(),
            ])
            .build();

        openapi::RefOr::T(openapi::schema::Schema::Object(schema))
    }
}
impl ToSchema for PermissionAction {}
