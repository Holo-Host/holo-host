use actix_web::{put, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use db_utils::schemas::{
    api_key::{ApiKey, API_KEY_COLLECTION_NAME},
    user_permissions::{PermissionAction, UserPermission},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

use super::apikey_dto::ApiKeyDto;

#[derive(OpenApi)]
#[openapi(paths(update_apikey), components(schemas(UpdateApiKeyDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UpdateApiKeyDto {
    /// The permissions to assign to the api key
    /// If you do not have the same permissions then the request will throw 403
    pub permissions: Vec<UserPermission>,
    /// When the Api key expires, this is a unix timestamp in seconds
    #[schema(example = 1672531199)]
    pub expire_at: i64,
    /// The description of the api key
    /// This is used to identify the api key
    #[schema(example = "my api key")]
    pub description: String,
}

#[utoipa::path(
    put,
    path = "/protected/v1/apikey/{id}",
    tag = "Apikey",
    summary = "Update API key",
    description = "Update details of a specific API key",
    security(
        ("Bearer" = [])
    ),
    params(
        ("id" = String, Path, description = "The ID of the API key")
    ),
    request_body = UpdateApiKeyDto,
    responses(
        (status = 200, body = ApiKeyDto)
    )
)]
#[put("/v1/apikey/{id}")]
pub async fn update_apikey(
    req: HttpRequest,
    id: web::Path<String>,
    db: web::Data<mongodb::Client>,
    payload: web::Json<UpdateApiKeyDto>,
) -> impl Responder {
    let id = id.into_inner();
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    // verify user can edit resource
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
                message: "Internal server error".to_string(),
            });
        }
    };
    if owner.is_none() {
        return HttpResponse::NotFound().json(ErrorResponse {
            message: "API key not found".to_string(),
        });
    }
    let owner = owner.unwrap();

    let permission_result = providers::auth::verify_all_permissions(
        claims.clone(),
        vec![UserPermission {
            resource: API_KEY_COLLECTION_NAME.to_string(),
            action: PermissionAction::Update,
            owner: owner.clone(),
            all_owners: false,
        }],
    );
    if !permission_result {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    // verify user has the permissions that are being set for the api key
    let permission_result = providers::auth::verify_all_permissions(
        claims.clone(),
        payload.permissions.clone(),
    );
    if !permission_result {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    let updates = bson::doc! {
        "permissions": bson::to_bson(&payload.permissions).unwrap_or_default(),
        "description": &payload.description,
        "expire_at": payload.expire_at,
    };
    match providers::crud::update::<ApiKey>(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        id.clone(),
        updates.clone(),
    )
    .await
    {
        Ok(update_result) => {
            tracing::error!("{:?}", update_result)
        }
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Internal server error".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(ApiKeyDto {
        id,
        owner: owner.clone(),
        permissions: payload.permissions.clone(),
        description: payload.description.clone(),
        expire_at: payload.expire_at,
    })
}
