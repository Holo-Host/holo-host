use actix_web::{get, HttpResponse, Responder};
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};


#[derive(OpenApi)]
#[openapi(
    paths(health_check),
    components(schemas(HealthCheckResponse))
)]
pub struct OpenApiSpec;

#[derive(Serialize, ToSchema)]
pub struct HealthCheckResponse {
    pub status: String,
}

#[utoipa::path(
    get,
    path = "/public/v1/general/health-check",
    tag = "General",
    summary = "Health check",
    description = "Health check",
    responses(
        (status = 200, body = HealthCheckResponse)
    )
)]
#[get("/v1/general/health-check")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(HealthCheckResponse{ status: "ok".to_string() })
}