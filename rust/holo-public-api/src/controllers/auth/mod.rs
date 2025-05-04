use actix_web::web;
use auth_dto::AuthLoginResponse;
use utoipa::OpenApi;
mod auth_dto;
mod login_apikey;
mod refresh;

#[derive(OpenApi)]
#[openapi(components(schemas(AuthLoginResponse)))]
pub struct AuthLoginResponseSpec;

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(login_apikey::login_with_apikey);
    cfg.service(refresh::refresh);
}

pub fn setup_private_controllers(_cfg: &mut web::ServiceConfig) {
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(AuthLoginResponseSpec::openapi());
    openapi.merge(login_apikey::OpenApiSpec::openapi());
    openapi.merge(refresh::OpenApiSpec::openapi());
    openapi
}
