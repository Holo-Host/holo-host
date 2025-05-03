use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use utoipa::OpenApi;

use crate::providers::config::AppConfig;

use super::auth_dto::AuthLoginResponse;

#[derive(OpenApi)]
#[openapi(paths(refresh))]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/public/v1/auth/refresh",
    tag = "Auth",
    summary = "Refresh access token",
    description = "Refresh the access token using the refresh token",
    security(
      ("Bearer" = [])
    ),
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[get("/v1/auth/refresh")]
pub async fn refresh(
    req: HttpRequest,
    config: web::Data<AppConfig>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    HttpResponse::Ok().json(AuthLoginResponse {
        access_token: "".to_string(),
        refresh_token: "".to_string(),
    })
}
