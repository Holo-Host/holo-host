use actix_web::web;
use utoipa::OpenApi;

pub mod login_response;
pub mod login_with_api_key;
pub mod login_refresh;
pub mod logout_all;
mod tests;

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg
    .service(logout_all::logout_all);
}

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    cfg
    .service(login_with_api_key::login_with_api_key)
    .service(login_refresh::login_refresh);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(login_with_api_key::OpenApiSpec::openapi());
    openapi.merge(login_refresh::OpenApiSpec::openapi());
    openapi.merge(logout_all::OpenApiSpec::openapi());
    openapi
}