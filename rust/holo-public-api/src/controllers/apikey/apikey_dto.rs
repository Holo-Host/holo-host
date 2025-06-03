use db_utils::schemas::user_permissions::UserPermission;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(components(schemas(ApiKeyDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ApiKeyDto {
    /// The unique id of this api key
    pub id: String,
    /// The owner (user id) of this api key
    pub owner: String,
    /// The permissions assigned to this api key
    pub permissions: Vec<UserPermission>,
    /// The description of this api key
    pub description: String,
    /// The expiration time (timestamp) of this api key
    pub expire_at: i64,
}

pub fn map_api_key_to_dto(api_key: db_utils::schemas::api_key::ApiKey) -> ApiKeyDto {
    ApiKeyDto {
        id: api_key._id.unwrap().to_hex(),
        owner: api_key.owner.to_string(),
        permissions: api_key.permissions.clone(),
        description: api_key.description.clone(),
        expire_at: api_key.expire_at,
    }
}
