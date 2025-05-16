use actix_web::{post, web, HttpResponse, Responder};
use bson::doc;
use db_utils::schemas::{
    api_key::{ApiKey, API_KEY_COLLECTION_NAME},
    user,
};
use utoipa::OpenApi;

use crate::providers::{
    self,
    config::AppConfig,
    error_response::ErrorResponse,
    jwt::{
        sign_access_token, sign_refresh_token, verify_access_token, verify_refresh_token,
        AccessTokenClaims, RefreshTokenClaims,
    },
};

use super::auth_dto::AuthLoginResponse;

#[derive(OpenApi)]
#[openapi(paths(refresh))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/public/v1/auth/refresh",
    tag = "Auth",
    summary = "Refresh access token",
    description = "Use this endpoint when your access token is expired to get a new one",
    request_body = AuthLoginResponse,
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/refresh")]
pub async fn refresh(
    payload: web::Json<AuthLoginResponse>,
    config: web::Data<AppConfig>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
    let config = config.get_ref();
    let current_time = bson::DateTime::now().to_chrono().timestamp() as usize;
    let mut refresh_token = payload.refresh_token.clone();
    let refresh_token_result = match verify_refresh_token(&refresh_token, &config.jwt_secret) {
        Ok(claims) => claims,
        Err(_) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "invalid refresh token".to_string(),
            })
        }
    };
    if refresh_token_result.exp < current_time {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "refresh token expired".to_string(),
        });
    }
    if refresh_token_result.allow_extending_refresh_token {
        refresh_token = match sign_refresh_token(
            RefreshTokenClaims {
                exp: bson::DateTime::now().to_chrono().timestamp() as usize + 60 + 60 * 24 * 30,
                sub: refresh_token_result.sub.clone(),
                reference_id: refresh_token_result.reference_id.clone(),
                allow_extending_refresh_token: true,
                version: refresh_token_result.version,
            },
            &config.jwt_secret,
        ) {
            Ok(result) => result,
            Err(error) => {
                tracing::error!("{}", error.to_string());
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "failed to sign refresh token".to_string(),
                });
            }
        };
    }
    let access_token_result = match verify_access_token(&payload.access_token, &config.jwt_secret) {
        Ok(claims) => claims,
        Err(_) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "invalid access token".to_string(),
            })
        }
    };
    if access_token_result.exp > current_time + 60 {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "access token is still valid for 60 seconds or longer".to_string(),
        });
    }
    if access_token_result.sub.clone() != refresh_token_result.sub.clone() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "access token does not match refresh token".to_string(),
        });
    }
    let user = match providers::crud::get::<user::User>(
        db.get_ref().clone(),
        user::USER_COLLECTION_NAME.to_string(),
        refresh_token_result.sub.clone(),
    )
    .await
    {
        Ok(value) => match value {
            None => {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    message: "user not found".to_string(),
                })
            }
            Some(value) => value,
        },
        Err(error) => {
            tracing::error!("{}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "failed to get user".to_string(),
            });
        }
    };
    if user.refresh_token_version != refresh_token_result.version {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "refresh token is invalid".to_string(),
        });
    }
    let mut permissions = user.permissions;
    if refresh_token_result.reference_id.is_some() {
        let api_key = match providers::crud::get::<ApiKey>(
            db.get_ref().clone(),
            API_KEY_COLLECTION_NAME.to_string(),
            refresh_token_result.reference_id.unwrap(),
        )
        .await
        {
            Ok(value) => value,
            Err(error) => {
                tracing::error!("{}", error);
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "failed to get user ID and permissions".to_string(),
                });
            }
        };
        if api_key.is_none() {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "invalid api key".to_string(),
            });
        }
        let api_key = api_key.unwrap();
        permissions = api_key.permissions;
    }

    let access_token = match sign_access_token(
        AccessTokenClaims {
            sub: access_token_result.sub,
            exp: bson::DateTime::now().to_chrono().timestamp() as usize
                + config.access_token_expiry.unwrap_or(300) as usize,
            permissions,
        },
        &config.jwt_secret,
    ) {
        Ok(value) => value,
        Err(error) => {
            tracing::error!("{}", error);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "failed to sign access token".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(AuthLoginResponse {
        access_token,
        refresh_token,
    })
}
