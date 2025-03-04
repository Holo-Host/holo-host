use actix_web::web;

pub mod auth;
pub mod general;

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    auth::setup_private_controllers(cfg);
}

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    general::setup_public_controllers(cfg);
    auth::setup_public_controllers(cfg);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(general::setup_docs());
    openapi.merge(auth::setup_docs());
    openapi
}
