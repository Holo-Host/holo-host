use actix_web::web;
use utoipa::OpenApi;

pub mod upload;

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(upload::upload_blob);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(upload::OpenApiSpec::openapi());
    openapi
}
