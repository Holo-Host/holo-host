use actix_web::web;

mod create_workload_layout;
mod workload_layout_dto;

pub fn setup_public_controllers(_cfg: &mut web::ServiceConfig) {}

pub fn setup_private_controllers(cfg: &mut web::ServiceConfig) {
    cfg.service(create_workload_layout::create_workload_layout);
}

pub fn setup_docs(internal: bool) -> utoipa::openapi::OpenApi {
    let mut openapi = utoipa::openapi::OpenApi::default();
    if internal {
        // openapi.merge(create_workload_layout::OpenApiSpec::openapi());
    }
    openapi
}
