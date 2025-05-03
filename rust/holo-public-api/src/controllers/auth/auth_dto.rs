use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct AuthLoginResponse {
    pub access_token: String,
    pub refresh_token: String,
}
