use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use super::auth_dto::AuthLoginResponse;
use crate::providers::{self, error_response::ErrorResponse};

#[derive(OpenApi)]
#[openapi(paths(register), components(schemas(RegisterRequestDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct RegisterRequestDto {
    given_names: String,
    family_name: String,
    email: String,
    password: String,
    jurisdiction: schemas::jurisdiction::Jurisdiction,
    email_verification_code: String,
}

#[utoipa::path(
    post,
    path = "/public/v1/auth/register",
    tag = "Auth",
    summary = "Register a new user",
    description = "Register a new account",
    request_body = RegisterRequestDto,
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/register")]
pub async fn register(
    req: HttpRequest,
    payload: web::Json<RegisterRequestDto>,
    db: web::Data<mongodb::Client>,
    cache: web::Data<deadpool_redis::Pool>,
) -> impl Responder {
    // todo: add cloudflare turnsite

    // verification before creating user
    let email_verify = match providers::crud::find_one::<schemas::email_verify::EmailVerify>(
        db.get_ref().clone(),
        schemas::email_verify::EMAIL_VERIFY_COLLECTION_NAME.to_string(),
        bson::doc! {
            "email": payload.email.clone(),
        },
    )
    .await
    {
        Ok(email_verify) => email_verify,
        Err(err) => {
            tracing::error!("failed to get email verify: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to get email verify".to_string(),
            });
        }
    };
    if email_verify.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "email not verified".to_string(),
        });
    }
    let email_verify = email_verify.unwrap();
    if email_verify.code != payload.email_verification_code {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "invalid email verification code".to_string(),
        });
    }
    match providers::crud::delete::<schemas::email_verify::EmailVerify>(
        db.get_ref().clone(),
        schemas::email_verify::EMAIL_VERIFY_COLLECTION_NAME.to_string(),
        email_verify._id.unwrap().to_hex(),
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

    let password_hash = match bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!("failed to hash password: {}", err);
            return HttpResponse::BadRequest().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to hash password".to_string(),
            });
        }
    };

    // check rate limiter
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

    // create user
    let user_id = match providers::crud::create(
        db.get_ref().clone(),
        schemas::user::USER_COLLECTION_NAME.to_string(),
        schemas::user::User {
            _id: None,
            developer: None,
            hoster: None,
            metadata: schemas::metadata::Metadata::default(),
            jurisdiction: "".to_string(),
            permissions: vec![schemas::user_permissions::UserPermission {
                resource: "all".to_string(),
                action: schemas::user_permissions::PermissionAction::All,
                owner: "self".to_string(),
            }],
            roles: vec![schemas::user::UserRole::User],
            refresh_token_version: 0,
        },
    )
    .await
    {
        Ok(id) => id,
        Err(err) => {
            tracing::error!("failed to create user: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to create user".to_string(),
            });
        }
    };
    match providers::crud::create(
        db.get_ref().clone(),
        schemas::user_password::USER_PASSWORD_COLLECTION_NAME.to_string(),
        schemas::user_password::UserPassword {
            _id: None,
            owner: user_id,
            password_hash,
            metadata: schemas::metadata::Metadata::default(),
        },
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("failed to create user password: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to create user password".to_string(),
            });
        }
    };
    match providers::crud::create(
        db.get_ref().clone(),
        schemas::user_info::USER_INFO_COLLECTION_NAME.to_string(),
        schemas::user_info::UserInfo {
            _id: None,
            user_id,
            metadata: schemas::metadata::Metadata::default(),
            email: payload.email.clone(),
            given_names: payload.given_names.clone(),
            family_name: payload.family_name.clone(),
            geographic_jurisdiction: payload.jurisdiction.clone(),
        },
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("failed to create user info: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to create user info".to_string(),
            });
        }
    }

    HttpResponse::Ok().json(bson::doc! {})
}
