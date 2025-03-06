use actix_web::web;
use utoipa::OpenApi;
mod get_workloads;
mod get_single_workload;
mod create_workload;
mod delete_workload;
mod update_workload;
mod tests;

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(get_workloads::get_workloads);
    cfg.service(get_single_workload::get_single_workload);
    cfg.service(create_workload::create_workload);
    cfg.service(delete_workload::delete_workload);
    cfg.service(update_workload::update_workload);
}

pub fn setup_docs() -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    openapi.merge(get_workloads::OpenApiSpec::openapi());
    openapi.merge(get_single_workload::OpenApiSpec::openapi());
    openapi.merge(create_workload::OpenApiSpec::openapi());
    openapi.merge(delete_workload::OpenApiSpec::openapi());
    openapi.merge(update_workload::OpenApiSpec::openapi());
    openapi
}