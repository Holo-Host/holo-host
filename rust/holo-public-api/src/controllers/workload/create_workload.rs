use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::{
    controllers::workload::workload_dto::from_manifest_dto,
    providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims},
};

use super::workload_dto::{WorkloadManifestDto, WorkloadManifestHolochainDhtV1Dto};

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
    manifest: WorkloadManifestHolochainDhtV1Dto,
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

    let mut workload = schemas::workload::Workload {
        ..Default::default()
    };
    workload.assigned_developer = match ObjectId::parse_str(payload.assigned_developer.clone()) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "Invalid developer ID".to_string(),
            });
        }
    };
    workload.manifest = from_manifest_dto(WorkloadManifestDto::HolochainDhtV1(Box::new(
        payload.manifest.clone(),
    )));
    let result = match providers::crud::create::<schemas::workload::Workload>(
        db.get_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        workload.clone(),
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
