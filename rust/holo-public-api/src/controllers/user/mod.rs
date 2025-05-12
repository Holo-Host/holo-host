use actix_web::web;
use utoipa::OpenApi;
mod create_user;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create_user::create_user);
}

pub fn setup_docs(internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    if internal {
        openapi.merge(create_user::OpenApiSpec::openapi());
    }
    openapi
}
