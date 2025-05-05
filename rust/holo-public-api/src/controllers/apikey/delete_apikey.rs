use actix_web::{delete, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas::{
    api_key::{ApiKey, API_KEY_COLLECTION_NAME},
    user_permissions::{PermissionAction, UserPermission},
};
use utoipa::OpenApi;

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(OpenApi)]
#[openapi(paths(delete_apikey))]
pub struct OpenApiSpec;

#[utoipa::path(
    delete,
    path = "/protected/v1/apikey/{id}",
    tag = "Apikey",
    summary = "Delete API key",
    description = "Delete a specific API key",
    security(
        ("Bearer" = [])
    ),
    params(
        ("id" = String, Path, description = "The ID of the API key")
    ),
    responses(
        (status = 200)
    )
)]
#[delete("/v1/apikey/{id}")]
pub async fn delete_apikey(
    req: HttpRequest,
    id: web::Path<String>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    // get claims for logged in user
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    // check owner of the resource
    let owner = match providers::crud::get_owner::<ApiKey>(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(owner) => owner,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "internal server error".to_string(),
            });
        }
    };
    if owner.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "api_key not found".to_string(),
        });
    }
    let owner = owner.unwrap();

    // verify user has permission to delete resource
    let permission_result = providers::auth::verify_all_permissions(
        claims.sub,
        claims.permissions,
        vec![UserPermission {
            resource: API_KEY_COLLECTION_NAME.to_string(),
            action: PermissionAction::Delete,
            owner,
            all_owners: false,
        }],
    );
    if !permission_result {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "you do not have access for this operation".to_string(),
        });
    }

    // delete resource
    match providers::crud::delete::<ApiKey>(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        id.clone(),
    )
    .await
    {
        Ok(_result) => HttpResponse::Ok().finish(),
        Err(error) => {
            tracing::error!("{:?}", error);
            HttpResponse::InternalServerError().json(ErrorResponse {
                message: "failed to delete resource".to_string(),
            })
        }
    }
}
