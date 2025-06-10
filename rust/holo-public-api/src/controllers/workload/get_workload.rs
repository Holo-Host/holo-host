use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::{
    controllers::workload::workload_dto::{WorkloadDto, WorkloadPropertiesDto},
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

    let id = id.into_inner();
    if id.is_empty() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "Workload not found".to_string(),
        });
    }

    let workload = match providers::crud::get::<schemas::workload::Workload>(
        db.get_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
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

    let developer = match providers::crud::get::<schemas::developer::Developer>(
        db.get_ref().clone(),
        schemas::developer::DEVELOPER_COLLECTION_NAME.to_string(),
        workload.assigned_developer.to_hex().clone(),
    )
    .await
    {
        Ok(developer) => developer,
        Err(e) => {
            tracing::error!("Error getting developer: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error getting developer".to_string(),
            });
        }
    };
    if developer.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "Developer not found".to_string(),
        });
    }
    let developer = developer.unwrap();

    // verify permissions
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

    let holochain_manifest = match &workload.manifest {
        schemas::workload::WorkloadManifest::HolochainDhtV1(inner) => Some(inner),
        _ => None,
    };
    if holochain_manifest.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Invalid workload manifest".to_string(),
        });
    }
    let holochain_manifest = holochain_manifest.unwrap();

    HttpResponse::Ok().json(WorkloadDto {
        id: workload._id.unwrap().to_hex(),
        properties: WorkloadPropertiesDto {
            instances: Some(workload.min_hosts),
            network_seed: holochain_manifest.network_seed.clone(),
            memproof: holochain_manifest.memproof.clone(),
            bootstrap_server_url: holochain_manifest
                .bootstrap_server_url
                .clone()
                .map(|url| url.to_string()),
            signal_server_url: holochain_manifest
                .signal_server_url
                .clone()
                .map(|url| url.to_string()),
            http_gw_allowed_fns: holochain_manifest.http_gw_allowed_fns.clone().map(|fns| {
                fns.into_iter()
                    .map(|url| url.to_string())
                    .collect::<Vec<String>>()
            }),
            http_gw_enable: holochain_manifest.http_gw_enable,
        },
        status: workload.status.actual,
    })
}
