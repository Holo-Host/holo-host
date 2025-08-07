use actix_web::web;
use auth_dto::AuthLoginResponse;
use utoipa::OpenApi;

mod auth_dto;
mod email_verify;
mod email_verify_check;
mod forgot_password;
mod login_apikey;
mod login_password;
mod refresh;
mod register;

#[cfg(test)]
mod tests;

#[derive(OpenApi)]
#[openapi(components(schemas(AuthLoginResponse)))]
pub struct AuthLoginResponseSpec;

pub fn setup_public_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(login_apikey::login_with_apikey);
    cfg.service(login_password::login_with_password);
    cfg.service(refresh::refresh);
    cfg.service(register::register);
    cfg.service(email_verify::email_verify);
    cfg.service(forgot_password::forgot_password);
    cfg.service(email_verify_check::email_verify_check);
}

pub fn setup_private_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_docs(internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(AuthLoginResponseSpec::openapi());
    openapi.merge(login_apikey::OpenApiSpec::openapi());
    if internal {
        openapi.merge(login_password::OpenApiSpec::openapi());
        openapi.merge(refresh::OpenApiSpec::openapi());
        openapi.merge(register::OpenApiSpec::openapi());
        openapi.merge(email_verify::OpenApiSpec::openapi());
        openapi.merge(forgot_password::OpenApiSpec::openapi());
        openapi.merge(email_verify_check::OpenApiSpec::openapi());
    }
    openapi
}
