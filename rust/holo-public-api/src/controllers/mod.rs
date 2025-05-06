use actix_web::web;
mod apikey;
mod auth;
mod blob;
pub mod general; // used for testing middleware
mod workload;

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    general::setup_public_controllers(cfg);
    auth::setup_public_controllers(cfg);
    apikey::setup_public_controllers(cfg);
    workload::setup_public_controllers(cfg);
    blob::setup_public_controllers(cfg);
}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    auth::setup_private_controllers(cfg);
    apikey::setup_private_controllers(cfg);
    workload::setup_private_controllers(cfg);
    blob::setup_private_controllers(cfg);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(general::setup_docs());
    openapi.merge(auth::setup_docs());
    openapi.merge(apikey::setup_docs());
    openapi.merge(workload::setup_docs());
    openapi.merge(blob::setup_docs());
    openapi
}
