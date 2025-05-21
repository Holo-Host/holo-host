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
    /// mongodb user id
    pub sub: String,
    /// how long until token expires
    pub exp: usize,
    /// this is used to invalidate previously generated access tokens
    pub version: i32,
    /// When auth/refresh is called should it extend the refresh token
    pub allow_extending_refresh_token: bool,
    /// mongodb id of api key collection.
    /// if refresh token was not created using an api key then it is None
    pub reference_id: Option<String>,
}
