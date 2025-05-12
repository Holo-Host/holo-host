use actix_web::web;
use utoipa::OpenApi;
mod apikey_dto;
mod create_apikey;
mod delete_apikey;
mod get_apikey;
mod get_multiple_apikey;
mod update_apikey;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create_apikey::create_api_key);
    cfg.service(get_apikey::get_api_key);
    cfg.service(get_multiple_apikey::get_multiple_apikey);
    cfg.service(update_apikey::update_apikey);
    cfg.service(delete_apikey::delete_apikey);
}

pub fn setup_docs(_internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(apikey_dto::OpenApiSpec::openapi());
    openapi.merge(create_apikey::OpenApiSpec::openapi());
    openapi.merge(get_apikey::OpenApiSpec::openapi());
    openapi.merge(get_multiple_apikey::OpenApiSpec::openapi());
    openapi.merge(update_apikey::OpenApiSpec::openapi());
    openapi.merge(delete_apikey::OpenApiSpec::openapi());
    openapi
}
