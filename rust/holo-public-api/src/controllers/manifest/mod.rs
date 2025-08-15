use actix_web::web;
use utoipa::OpenApi;

mod create_manifest;
mod delete_manifest;
mod get_manifest;
mod manifest_dto;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create_manifest::create_manifest);
    cfg.service(get_manifest::get_manifest);
    cfg.service(delete_manifest::delete_manifest);
}

pub fn setup_docs(internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    if internal {
        openapi.merge(create_manifest::OpenApiSpec::openapi());
        openapi.merge(get_manifest::OpenApiSpec::openapi());
        openapi.merge(delete_manifest::OpenApiSpec::openapi());
    }
    openapi
}
