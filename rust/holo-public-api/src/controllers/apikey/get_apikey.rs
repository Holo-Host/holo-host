use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas::{
    api_key::{ApiKey, API_KEY_COLLECTION_NAME},
    user_permissions::{PermissionAction, UserPermission},
};
use utoipa::OpenApi;

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

use super::apikey_dto::ApiKeyDto;

#[derive(OpenApi)]
#[openapi(paths(get_api_key))]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/protected/v1/apikey/{id}",
    tag = "Apikey",
    summary = "Get API key",
    description = "Get details of a specific API key",
    security(
        ("Bearer" = [])
    ),
    params(
        ("id" = String, Path, description = "The ID of the API key")
    ),
    responses(
        (status = 200, body = ApiKeyDto)
    )
)]
#[get("/v1/apikey/{id}")]
pub async fn get_api_key(
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

    let api_key_result = match providers::crud::get::<ApiKey>(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        id.into_inner(),
    )
    .await
    {
        Ok(api_key) => api_key,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };
    if api_key_result.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "API key not found".to_string(),
        });
    }
    let api_key_result = api_key_result.unwrap();

    let permission_result = providers::auth::verify_all_permissions(
        claims.clone(),
        vec![UserPermission {
            resource: API_KEY_COLLECTION_NAME.to_string(),
            action: PermissionAction::Read,
            owner: api_key_result.owner.to_hex(),
            all_owners: false,
        }],
    );
    if !permission_result {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let oid = api_key_result._id.unwrap().to_hex();
    HttpResponse::Ok().json(ApiKeyDto {
        id: oid,
        owner: api_key_result.owner.to_hex(),
        permissions: api_key_result.permissions,
        description: api_key_result.description,
        expire_at: api_key_result.expire_at,
    })
}
