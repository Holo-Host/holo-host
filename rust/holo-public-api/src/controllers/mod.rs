pub mod general; // used for testing middleware
use actix_web::web;
mod apikey;
mod auth;
mod blob;
mod manifest;
mod user;
mod workload;

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    general::setup_public_controllers(cfg);
    auth::setup_public_controllers(cfg);
    apikey::setup_public_controllers(cfg);
    user::setup_public_controllers(cfg);
    workload::setup_public_controllers(cfg);
    manifest::setup_public_controllers(cfg);
    blob::setup_public_controllers(cfg);
}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    auth::setup_private_controllers(cfg);
    apikey::setup_private_controllers(cfg);
    workload::setup_private_controllers(cfg);
    manifest::setup_private_controllers(cfg);
    blob::setup_private_controllers(cfg);
    user::setup_private_controllers(cfg);
}

pub fn setup_docs(internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(general::setup_docs(internal));
    openapi.merge(auth::setup_docs(internal));
    openapi.merge(apikey::setup_docs(internal));
    openapi.merge(workload::setup_docs(internal));
    openapi.merge(manifest::setup_docs(internal));
    openapi.merge(blob::setup_docs(internal));
    openapi.merge(user::setup_docs(internal));
    openapi
}
