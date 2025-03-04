use actix_web::{post, web, HttpResponse, Responder};
use chrono::{Duration, Utc};
use mongodb::Database;
use serde_json::json;
use utoipa::OpenApi;
use crate::providers::{self, database::schemas::{user::User, user::USER_COLLECTION_NAME}, jwt::{AccessTokenClaims, RefreshTokenClaims}};

use super::login_response::LoginResponse;

#[utoipa::path(
    post,
    path = "/public/v1/auth/login-refresh",
    tag = "Auth",
    summary = "Refresh an expired access token",
    description = "Refresh an expired access token",
    request_body(
        content = LoginResponse
    ),
    responses(
        (status = 200, body = LoginResponse)
    )
)]
#[post("v1/auth/login-refresh")]
pub async fn login_refresh(
    body: web::Json<LoginResponse>,
    config: web::Data<providers::config::AppConfig>,
    db: web::Data<Database>
) -> impl Responder {   
    let access_token = body.access_token.clone();
    let refresh_token = body.refresh_token.clone();

    // verify expired access token
    let access_token_claims = match providers::jwt::verify_access_token(
        &access_token, &config.jwt_secret
    ) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::debug!("Error verifying access token: {}", err);
            return HttpResponse::Unauthorized().json(json!({ "error": "Invalid access token" }));
        }
    };

    // verify refresh token
    let refresh_token_claims = match providers::jwt::verify_refresh_token(
        &refresh_token, &config.jwt_secret
    ) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::debug!("Error verifying refresh token: {}", err);
            return HttpResponse::Unauthorized().json(json!({ "error": "Invalid refresh token" }));
        }
    };
    // check if refresh token is expired
    if refresh_token_claims.exp < chrono::Utc::now().timestamp() as usize {
        return HttpResponse::Unauthorized().json(json!({ "error": "Refresh token expired" }));
    }

    // verify access token and refresh token are for the same user
    if access_token_claims.sub != refresh_token_claims.sub {
        return HttpResponse::Unauthorized().json(json!({ "error": "Invalid access token" }));
    }

    // verify user from database
    let user = match db.collection::<User>(
        USER_COLLECTION_NAME
    ).find_one(
        bson::doc!{
            "_id": bson::oid::ObjectId::parse_str(&access_token_claims.sub).unwrap()
        }, None
    ).await {
        Ok(user) => user,
        Err(err) => {
            tracing::error!("Error finding user: {}", err);
            return HttpResponse::Unauthorized().json(json!({ "error": "Invalid access token" }));
        }
    };
    if user.is_none() {
        return HttpResponse::Unauthorized().json(json!({ "error": "Invalid access token" }));
    }
    let user = user.unwrap();

    // verify refresh token version
    if user.refresh_token_version != refresh_token_claims.version {
        return HttpResponse::Unauthorized().json(json!({ "error": "Invalid refresh token" }));
    }

    // sign new access token
    let access_token = providers::jwt::sign_access_token(AccessTokenClaims {
        sub: user._id.map(|id| id.to_string()).unwrap(),
        exp: (Utc::now() + Duration::minutes(5)).timestamp() as usize,
        permissions: user.permissions,
    }, &config.jwt_secret).unwrap();

    // sign new refresh token
    let refresh_token = providers::jwt::sign_refresh_token(RefreshTokenClaims {
        sub: user._id.map(|id| id.to_string()).unwrap(),
        exp: (Utc::now() + Duration::days(7)).timestamp() as usize,
        version: user.refresh_token_version,
    }, &config.jwt_secret).unwrap();

    HttpResponse::Ok().json(LoginResponse {
        access_token,
        refresh_token,
    })
}

#[derive(OpenApi)]
#[openapi(
    paths(login_refresh)
)]
pub struct OpenApiSpec;