use actix_web::web;
use utoipa::OpenApi;
mod create_workload;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create_workload::create_workload);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(create_workload::OpenApiSpec::openapi());
    openapi
}
