use actix_web::{delete, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateWorkloadResponse {
    pub id: String,
}

#[derive(OpenApi)]
#[openapi(paths(delete_workload))]
pub struct OpenApiSpec;

#[utoipa::path(
    delete,
    path = "/protected/v1/workload/{id}",
    tag = "Workload",
    summary = "Delete workload",
    description = "Requires 'workload.Delete' permission",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200)
    )
)]
#[delete("/v1/workload/{id}")]
pub async fn delete_workload(
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

    // get workload
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

    // get developer
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
            action: schemas::user_permissions::PermissionAction::Delete,
            owner: developer.user_id.to_hex(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    // delete workload
    match providers::crud::delete::<schemas::workload::Workload>(
        db.get_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        workload._id.unwrap().to_hex(),
    )
    .await
    {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Error deleting workload: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Error deleting workload".to_string(),
            });
        }
    }

    HttpResponse::Ok().finish()
}
