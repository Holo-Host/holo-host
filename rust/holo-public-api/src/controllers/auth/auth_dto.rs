use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AuthLoginResponse {
    pub access_token: String,
    pub refresh_token: String,
}
