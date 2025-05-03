use db_utils::schemas::user_permissions::UserPermission;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessTokenClaims {
    /// mongodb user id
    pub sub: String,
    /// how long until the token expires
    pub exp: usize,
    /// user permissions for the token
    pub permissions: Vec<UserPermission>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefreshTokenClaims {
    pub sub: String,
    pub exp: usize,
    pub version: i32,
    pub allow_extending_refresh_token: bool,
}
