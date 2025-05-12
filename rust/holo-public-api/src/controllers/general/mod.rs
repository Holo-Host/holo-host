use actix_web::web;
use utoipa::OpenApi;

pub mod health_check;

pub mod tests;

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(health_check::health_check);
}

pub fn setup_docs(_internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(health_check::OpenApiSpec::openapi());
    openapi
}
