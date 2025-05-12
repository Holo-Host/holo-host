use actix_web::web;
use utoipa::OpenApi;
mod create_workload;
mod delete_workload;
mod get_workload;
mod workload_dto;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create_workload::create_workload);
    cfg.service(get_workload::get_workload);
    cfg.service(delete_workload::delete_workload);
}

pub fn setup_docs(internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    if internal {
        openapi.merge(workload_dto::OpenApiSpec::openapi());
        openapi.merge(create_workload::OpenApiSpec::openapi());
        openapi.merge(get_workload::OpenApiSpec::openapi());
        openapi.merge(delete_workload::OpenApiSpec::openapi());
    }
    openapi
}
