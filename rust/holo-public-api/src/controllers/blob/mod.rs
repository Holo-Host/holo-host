use actix_web::web;
use utoipa::OpenApi;
mod create;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create::create_blob);
}

pub fn setup_docs(_internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(create::OpenApiSpec::openapi());
    openapi
}
