use actix_web::{
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    HttpResponse,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
}
pub fn create_middleware_error_response(
    req: ServiceRequest,
    status: StatusCode,
    message: &str,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let (req_http, _) = req.into_parts();
    let resp = HttpResponse::build(status)
        .json(ErrorResponse {
            message: message.to_string(),
        })
        .map_into_boxed_body();
    Ok(ServiceResponse::new(req_http, resp))
}
