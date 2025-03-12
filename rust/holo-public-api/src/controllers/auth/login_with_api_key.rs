use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use chrono::{Duration, Utc};
use mongodb::Database;
use crate::providers::{self, database::{self, schemas}, permissions::get_user_permissions};

use super::login_response::LoginResponse;
use serde_json::json;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(login_with_api_key),
    components(schemas(LoginResponse))
)]
pub struct OpenApiSpec;

#[derive(serde::Deserialize)]
pub struct ApiKeyWithUser {
    pub user: schemas::user::User,
}

#[utoipa::path(
    post,
    path = "/public/v1/auth/login-with-api-key",
    tag = "Auth",
    summary = "Login with API key",
    description = "Login with API key",
    security(
        ("ApiKey" = [])
    ),
    responses(
        (status = 200, body = LoginResponse)
    )
)]
#[post("v1/auth/login-with-api-key")]
pub async fn login_with_api_key(
    request: HttpRequest,
    config: web::Data<providers::config::AppConfig>,
    db: web::Data<Database>
) -> impl Responder {
    let api_key = match request.headers().get("API-KEY") {
        Some(api_key) => api_key.to_str().unwrap(),
        None => {
            return HttpResponse::BadRequest().json(json!({ "error": "API key is required" }));
        }
    };

    if api_key.is_empty() {
        return HttpResponse::BadRequest().json(json!({ "error": "API key is required" }));
    }

    let api_key_cursor = match db.collection::<schemas::api_key::ApiKey>(
        schemas::api_key::API_KEY_COLLECTION_NAME
    ).aggregate(vec![
        doc! {
            "$match": {
                "key": api_key
            }
        },
        doc! {
            "$lookup": {
                "from": "users",
                "localField": "user_id",
                "foreignField": "_id",
                "as": "user"
            }
        },
        doc! {
            "$unwind": "$user"
        },
        doc! {
            "$project": {
                "_id": 1,
                "user": 1,
            }
        }
    ], None).await {
        Ok(cursor) => cursor,
        Err(e) => {
            tracing::error!("Error logging in with API key: {}", e);
            return HttpResponse::InternalServerError().json(json!({ "error": "Failed to login with API key" }));
        }
    };
    let api_key = match database::cursor_to_vec::<ApiKeyWithUser>(api_key_cursor).await {
        Ok(api_key) => api_key,
        Err(e) => {
            tracing::error!("Error logging in with API key: {}", e);
            return HttpResponse::InternalServerError().json(json!({ "error": "Failed to login with API key" }));
        }
    };

    if api_key.is_empty() {
        return HttpResponse::Unauthorized().json(json!({ "error": "Invalid API key" }));
    }

    let api_key = api_key.first().unwrap();
    let user_id = api_key.user.oid.map(|id| id.to_string()).unwrap();
    let permissions = get_user_permissions(api_key.user.clone());

    let access_token_exp = Utc::now() + Duration::minutes(5);
    let refresh_token_exp = Utc::now() + Duration::days(7);

    let access_token = providers::jwt::sign_jwt(providers::jwt::AccessTokenClaims {
        sub: user_id.clone(),
        exp: access_token_exp.timestamp() as usize,
        permissions,
    }, &config.jwt_secret).unwrap();

    let refresh_token = providers::jwt::sign_jwt(providers::jwt::RefreshTokenClaims {
        sub: user_id,
        exp: refresh_token_exp.timestamp() as usize,
        version: api_key.user.refresh_token_version,
    }, &config.jwt_secret).unwrap();

    let response = LoginResponse {
        access_token,
        refresh_token,
    };

    HttpResponse::Ok()
        .content_type("application/json")
        .json(response)
}