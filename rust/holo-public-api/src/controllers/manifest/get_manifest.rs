use crate::{controllers::manifest::manifest_dto, providers};
use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(get_manifest))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/manifest",
    tag = "Manifest",
    summary = "Create manifiest",
    description = "Requires 'manifest.Read' permission",
    security(
        ("Bearer" = [])
    ),
    responses(
        (status = 200, body = manifest_dto::ManifestDto)
    )
)]
#[get("/v1/manifest/{id}")]
pub async fn get_manifest(
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

    let owner = match providers::crud::get_owner::<schemas::manifest::Manifest>(
        db.get_ref().clone(),
        schemas::manifest::MANIFEST_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to get manifest".to_string(),
                },
            );
        }
    };
    if owner.is_none() {
        return HttpResponse::NotFound().json(providers::error_response::ErrorResponse {
            message: "no manifest found with the given id".to_string(),
        });
    }
    let owner = owner.unwrap();

    // Verify permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::manifest::MANIFEST_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Read,
            owner,
        }],
    ) {
        return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let result = match providers::crud::get::<schemas::manifest::Manifest>(
        db.as_ref().clone(),
        schemas::manifest::MANIFEST_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to create manifest".to_string(),
                },
            );
        }
    };
    if result.is_none() {
        return HttpResponse::NotFound().json(providers::error_response::ErrorResponse {
            message: "no manifest found with the given id".to_string(),
        });
    }
    let result = result.unwrap();

    HttpResponse::Ok().json(manifest_dto::ManifestDto {
        id: result._id.unwrap().to_hex(),
        name: result.name,
        tag: result.tag,
        workload_type: manifest_dto::workload_type_to_dto(result.workload_type),
    })
}
