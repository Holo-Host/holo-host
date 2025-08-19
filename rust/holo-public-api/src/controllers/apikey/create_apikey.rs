use actix_web::{post, web, HttpMessage, HttpRequest, HttpResponse, Responder};
use bson::oid::ObjectId;
use db_utils::schemas::{
    api_key::{ApiKey, API_KEY_COLLECTION_NAME},
    user_permissions::{PermissionAction, UserPermission},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::providers::{self, error_response::ErrorResponse, jwt::AccessTokenClaims};

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    /// The permissions to assign to the api key
    /// If you do not have the same permissions then the request will throw 403
    pub permissions: Vec<UserPermission>,
    /// What version of the api key to create
    /// This will be used to hash the api key (v0 = plain text)
    #[schema(example = "v0")]
    pub version: String,
    /// When the Api key expires, this is a unix timestamp in seconds
    #[schema(example = 1672531199)]
    pub expire_at: i64,
    /// The description of the api key
    /// This is used to identify the api key
    #[schema(example = "my api key")]
    pub description: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub api_key: String,
}

#[derive(OpenApi)]
#[openapi(paths(create_api_key))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/protected/v1/apikey",
    tag = "Apikey",
    summary = "Create API key",
    description = "Requires 'api_key.Create' permission",
    security(
        ("Bearer" = [])
    ),
    request_body = CreateApiKeyRequest,
    responses(
        (status = 200, body = CreateApiKeyResponse)
    )
)]
#[post("/v1/apikey")]
pub async fn create_api_key(
    req: HttpRequest,
    payload: web::Json<CreateApiKeyRequest>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    // get current user claims (permissions and user info)
    let claims = req.extensions().get::<AccessTokenClaims>().cloned();
    if claims.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Unauthorized".to_string(),
        });
    }
    let claims = claims.unwrap();

    // verify user permissions
    if !providers::auth::verify_all_permissions(
        claims.clone(),
        vec![UserPermission {
            resource: API_KEY_COLLECTION_NAME.to_string(),
            action: PermissionAction::Create,
            owner: claims.sub.clone(),
        }],
    ) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }
    if !providers::auth::verify_all_permissions(claims.clone(), payload.permissions.clone()) {
        return HttpResponse::Forbidden().json(ErrorResponse {
            message: "Permission denied".to_string(),
        });
    }

    // generate api key
    let api_key = providers::auth::generate_api_key();
    let api_key_hash = match providers::auth::hash_apikey(payload.version.clone(), api_key.clone())
    {
        Some(hash) => hash,
        None => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "invalid api key version".to_string(),
            });
        }
    };

    // create api key in db
    let owner_oid = match ObjectId::parse_str(claims.sub.clone()) {
        Ok(oid) => oid,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "invalid owner id".to_string(),
            });
        }
    };
    let result = match providers::crud::create(
        db.get_ref().clone(),
        API_KEY_COLLECTION_NAME.to_string(),
        ApiKey {
            _id: ObjectId::new(),
            metadata: db_utils::schemas::metadata::Metadata::default(),
            api_key: api_key_hash,
            owner: owner_oid,
            permissions: payload.permissions.clone(),
            description: payload.description.clone(),
            expire_at: payload.expire_at,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("{:?}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to create api key".to_string(),
            });
        }
    };

    // return api key
    HttpResponse::Ok().json(CreateApiKeyResponse {
        id: result.to_hex(),
        api_key: format!("{}-{}", payload.version, api_key),
    })
}
