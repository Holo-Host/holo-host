use crate::{controllers::manifest::manifest_dto, providers};
use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(create_manifest))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/manifest",
    tag = "Manifest",
    summary = "Create manifiest",
    description = "Requires 'manifest.Create' permission",
    security(
        ("Bearer" = [])
    ),
    request_body = manifest_dto::CreateManifestDto,
    responses(
        (status = 200, body = manifest_dto::ManifestDto)
    )
)]
#[post("/v1/manifest")]
pub async fn create_manifest(
    req: HttpRequest,
    payload: web::Json<manifest_dto::ManifestDto>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let payload = payload.into_inner();
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
    let user_id = match ObjectId::parse_str(claims.sub.clone()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("{:?}", e);
            return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
                message: "Permission denied".to_string(),
            });
        }
    };

    // Verify permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![schemas::user_permissions::UserPermission {
            resource: schemas::manifest::MANIFEST_COLLECTION_NAME.to_string(),
            action: schemas::user_permissions::PermissionAction::Create,
            owner: claims.sub,
        }],
    ) {
        return HttpResponse::Forbidden().json(providers::error_response::ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let manifest_type = match manifest_dto::manifest_type_from_dto(payload.manifest_type.clone()) {
        Ok(manifest_type) => manifest_type,
        Err(e) => {
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: e.to_string(),
                },
            );
        }
    };

    match providers::crud::create(
        db.as_ref().clone(),
        schemas::manifest::MANIFEST_COLLECTION_NAME.to_string(),
        schemas::manifest::Manifest {
            _id: ObjectId::new(),
            metadata: schemas::metadata::Metadata::default(),
            owner: user_id,
            name: payload.name.clone(),
            tag: payload.tag.clone(),
            manifest_type,
        },
    )
    .await
    {
        Ok(result) => {
            return HttpResponse::Ok().json(manifest_dto::ManifestDto {
                id: result.to_hex(),
                name: payload.name,
                tag: payload.tag,
                manifest_type: payload.manifest_type,
            })
        }
        Err(err) => {
            tracing::error!("{:?}", err);
            return HttpResponse::InternalServerError().json(
                providers::error_response::ErrorResponse {
                    message: "failed to create manifest".to_string(),
                },
            );
        }
    }
}
