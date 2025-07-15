use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use db_utils::{mongodb::traits::WithMongoDbId, schemas};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use super::auth_dto::AuthLoginResponse;
use crate::providers::{self, crud, error_response::ErrorResponse};

#[derive(OpenApi)]
#[openapi(paths(forgot_password), components(schemas(ForgotPasswordRequestDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ForgotPasswordRequestDto {
    pub email: String,
    pub email_confirmation_code: String,
    pub new_password: String,
}

#[utoipa::path(
    post,
    path = "/public/v1/auth/forgot-password",
    tag = "Auth",
    summary = "Forgot Password",
    description = "Can be used to reset the password of an existing user",
    request_body = ForgotPasswordRequestDto,
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/forgot-password")]
pub async fn forgot_password(
    req: HttpRequest,
    payload: web::Json<ForgotPasswordRequestDto>,
    db: web::Data<mongodb::Client>,
    cache: web::Data<deadpool_redis::Pool>,
) -> impl Responder {
    match providers::limiter::limiter_by_ip(
        cache,
        req.clone(),
        providers::limiter::LimiterOptions {
            rate_limit_max_requests: 3,
            rate_limit_window: 300,
        },
    )
    .await
    {
        true => {}
        false => {
            return HttpResponse::TooManyRequests().json(ErrorResponse {
                message: "rate limit exceeded".to_string(),
            });
        }
    };
    let user_info = match providers::crud::find_one::<schemas::user_info::UserInfo>(
        db.get_ref().clone(),
        schemas::user_info::USER_INFO_COLLECTION_NAME.to_string(),
        bson::doc! {
            "email": payload.email.clone()
        },
    )
    .await
    {
        Ok(user_info) => {
            if user_info.is_none() {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    message: "User does not exist".to_string(),
                });
            }
            user_info.unwrap()
        }
        Err(err) => {
            tracing::error!("failed to get user info: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to get user info".to_string(),
            });
        }
    };

    let email_verify = match providers::crud::find_one::<schemas::email_verify::EmailVerify>(
        db.get_ref().clone(),
        schemas::email_verify::EMAIL_VERIFY_COLLECTION_NAME.to_string(),
        bson::doc! {
            "email": payload.email.clone(),
        },
    )
    .await
    {
        Ok(email_verify) => {
            if email_verify.is_none() {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    message: "Email verification code is invalid".to_string(),
                });
            }
            email_verify.unwrap()
        }
        Err(err) => {
            tracing::error!("failed to get email verify: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to get email verify".to_string(),
            });
        }
    };

    if email_verify.code != payload.email_confirmation_code {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Email verification code is invalid".to_string(),
        });
    }
    if email_verify.email != payload.email {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Email is invalid".to_string(),
        });
    }
    let password_hash = match bcrypt::hash(payload.new_password.clone(), bcrypt::DEFAULT_COST) {
        Ok(hashed_password) => hashed_password,
        Err(err) => {
            tracing::error!("failed to hash password: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to hash password".to_string(),
            });
        }
    };

    match crud::upsert::<schemas::user_password::UserPassword>(
        db.get_ref().clone(),
        schemas::user_password::USER_PASSWORD_COLLECTION_NAME.to_string(),
        bson::doc! {
            "owner": user_info.user_id,
        },
        schemas::user_password::UserPassword {
            _id: None,
            owner: user_info.user_id,
            password_hash,
            metadata: schemas::metadata::Metadata::default(),
        },
    )
    .await
    {
        Ok(oid) => oid,
        Err(err) => {
            tracing::error!("failed to create user password: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to create user password".to_string(),
            });
        }
    };

    match crud::delete_hard::<schemas::email_verify::EmailVerify>(
        db.get_ref().clone(),
        schemas::email_verify::EMAIL_VERIFY_COLLECTION_NAME.to_string(),
        email_verify.get_id_string(),
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("failed to delete email verify: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to delete email verify".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(bson::doc! {})
}
