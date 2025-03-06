use actix_web::web;

pub mod general;

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {

}

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    general::setup_public_controllers(cfg);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(general::setup_docs());
    openapi
}
