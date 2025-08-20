use crate::providers;
use actix_web::{delete, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use utoipa::OpenApi;

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
    let claims = req
        .extensions()
        .get::<providers::jwt::AccessTokenClaims>()
        .cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(providers::error_response::ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    let owner = match providers::crud::get_owner::<schemas::workload::Workload>(
        db.get_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to get workload".to_string(),
                },
            );
        }
    };
    if owner.is_none() {
        return HttpResponse::NotFound().json(providers::error_response::ErrorResponse {
            message: "no workload found with the given id".to_string(),
        });
    }
    let owner = owner.unwrap();

    // Verify permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Delete,
            owner,
        }],
    ) {
        return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let _ = match providers::crud::delete::<schemas::workload::Workload>(
        db.as_ref().clone(),
        schemas::workload::WORKLOAD_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to delete workload".to_string(),
                },
            );
        }
    };

    HttpResponse::Ok().json(bson::doc! {})
}
