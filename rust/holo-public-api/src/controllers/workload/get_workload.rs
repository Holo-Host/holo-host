use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::{
    controllers::workload::workload_dto::{to_workload_dto, WorkloadDto},
    providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims},
};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateWorkloadResponse {
    pub id: String,
}

#[derive(OpenApi)]
#[openapi(paths(get_workload))]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/protected/v1/workload/{id}",
    tag = "Workload",
    summary = "Get workload",
    description = "Requires 'workload.Read' permission",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, body = WorkloadDto)
    )
)]
#[get("/v1/workload/{id}")]
pub async fn get_workload(
    req: HttpRequest,
    id: web::Path<String>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    // verify permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::workload_layout::WORKLOAD_LAYOUT_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Read,
            owner: claims.sub.clone(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let id = id.into_inner();
    if id.is_empty() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "Workload not found".to_string(),
        });
    }

    let workload = match providers::crud::get::<schemas::workload_layout::WorkloadLayout>(
        db.get_ref().clone(),
        schemas::workload_layout::WORKLOAD_LAYOUT_COLLECTION_NAME.to_string(),
        id.to_string().clone(),
    )
    .await
    {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("Error getting workload: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error getting workload".to_string(),
            });
        }
    };
    if workload.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "Workload not found".to_string(),
        });
    }
    let workload = workload.unwrap();
    let workload_dto = to_workload_dto(workload.clone());

    HttpResponse::Ok().json(workload_dto)
}
