use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AuthLoginResponse {
    /// This is used to authenticate the user for each request
    /// Access token should be short lived (usually 5 minutes)
    pub access_token: String,
    /// This is used to refresh the access token after access token expires
    pub refresh_token: String,
}
