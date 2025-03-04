#[derive(utoipa::ToSchema, serde::Serialize, serde::Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
}