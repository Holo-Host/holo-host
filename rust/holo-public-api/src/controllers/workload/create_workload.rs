use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::{
    controllers::workload::workload_dto::{from_workload_dto, WorkloadDto},
    providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims},
};

use super::workload_dto::{SystemSpecsDto, WorkloadManifestDto, WorkloadStatusDto};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateWorkloadResponse {
    pub id: String,
}

#[derive(OpenApi)]
#[openapi(paths(create_workload))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct CreateWorkloadRequest {
    assigned_developer: String,
    version: String,
    min_hosts: i32,
    system_specs: SystemSpecsDto,
    assigned_hosts: Vec<String>,
    status: WorkloadStatusDto,
    manifest: WorkloadManifestDto,
}

#[utoipa::path(
    post,
    path = "/protected/v1/workload",
    tag = "Workload",
    summary = "Create workload",
    description = "Requires 'workload.Create' permission",
    security(
        ("Bearer" = [])
    ),
    request_body = CreateWorkloadRequest,
    responses(
        (status = 200, body = CreateWorkloadResponse)
    )
)]
#[post("/v1/workload")]
pub async fn create_workload(
    req: HttpRequest,
    payload: web::Json<CreateWorkloadRequest>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    let developer = match providers::crud::get::<schemas::developer::Developer>(
        db.get_ref().clone(),
        schemas::developer::DEVELOPER_COLLECTION_NAME.to_string(),
        payload.assigned_developer.clone(),
    )
    .await
    {
        Ok(developer) => developer,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "internal server error".to_string(),
            });
        }
    };
    if developer.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "developer not found".to_string(),
        });
    }

    let developer = developer.unwrap();
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::developer::DEVELOPER_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Read,
            owner: developer.user_id.to_hex(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let result = match providers::crud::create::<schemas::workload::Workload>(
        db.get_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        from_workload_dto(WorkloadDto {
            id: None,
            assigned_developer: payload.assigned_developer.clone(),
            assigned_hosts: payload.assigned_hosts.clone(),
            min_hosts: payload.min_hosts,
            status: payload.status.clone(),
            version: payload.version.clone(),
            system_specs: payload.system_specs.clone(),
            manifest: payload.manifest.clone(),
        }),
    )
    .await
    {
        Ok(workload) => workload,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "internal server error".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(CreateWorkloadResponse {
        id: result.to_hex(),
    })
}
