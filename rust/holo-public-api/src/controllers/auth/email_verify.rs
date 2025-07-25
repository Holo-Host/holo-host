use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use db_utils::schemas;
use rand::Rng;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use super::auth_dto::AuthLoginResponse;
use crate::providers::{self, error_response::ErrorResponse};

#[derive(OpenApi)]
#[openapi(paths(email_verify), components(schemas(EmailVerifyRequestDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct EmailVerifyRequestDto {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    check_account_exists: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_url: Option<String>,
}

#[utoipa::path(
    post,
    path = "/public/v1/auth/email-verify",
    tag = "Auth",
    summary = "Verify Email",
    description = "Some endpoints require the user to verify their email address before they can be used. This endpoint starts the email verification by sending the email a code that can be used to verify the email address.",
    request_body = EmailVerifyRequestDto,
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/email-verify")]
pub async fn email_verify(
    req: HttpRequest,
    payload: web::Json<EmailVerifyRequestDto>,
    db: web::Data<mongodb::Client>,
    cache: web::Data<deadpool_redis::Pool>,
    config: web::Data<providers::config::AppConfig>,
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
    if payload.check_account_exists == Some(true) {
        match providers::crud::find_one::<schemas::user_info::UserInfo>(
            db.get_ref().clone(),
            schemas::user_info::USER_INFO_COLLECTION_NAME.to_string(),
            bson::doc! {
                "email": payload.email.clone()
            },
        )
        .await
        {
            Ok(user_info) => {
                if user_info.is_some() {
                    return HttpResponse::BadRequest().json(ErrorResponse {
                        message: "A user already exists with this email address".to_string(),
                    });
                }
            }
            Err(err) => {
                tracing::error!("failed to get user info: {}", err);
                return HttpResponse::InternalServerError().json(bson::doc! {
                    "error": err.to_string(),
                    "message": "failed to get user info".to_string(),
                });
            }
        };
    }
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
    let code = rand::rng().random_range(100_000..1_000_000).to_string();
    if email_verify.is_none() {
        match providers::crud::create(
            db.get_ref().clone(),
            schemas::email_verify::EMAIL_VERIFY_COLLECTION_NAME.to_string(),
            schemas::email_verify::EmailVerify {
                _id: None,
                email: payload.email.clone(),
                code: code.clone(),
                metadata: schemas::metadata::Metadata::default(),
            },
        )
        .await
        {
            Ok(_) => {}
            Err(err) => {
                tracing::error!("failed to create email verify: {}", err);
                return HttpResponse::InternalServerError().json(bson::doc! {
                    "error": err.to_string(),
                    "message": "failed to create email verify".to_string(),
                });
            }
        }
    } else {
        match providers::crud::update::<schemas::email_verify::EmailVerify>(
            db.get_ref().clone(),
            schemas::email_verify::EMAIL_VERIFY_COLLECTION_NAME.to_string(),
            email_verify.unwrap()._id.unwrap().to_hex(),
            bson::doc! {
                "code": code.clone(),
            },
        )
        .await
        {
            Ok(_) => {}
            Err(err) => {
                tracing::error!("failed to update email verify: {}", err);
                return HttpResponse::InternalServerError().json(bson::doc! {
                    "error": err.to_string(),
                    "message": "failed to update email verify".to_string(),
                });
            }
        }
    }

    match providers::postmark::send_email(
        config
            .postmark_api_key
            .clone()
            .expect("postmark api key not set"),
        payload.email.clone(),
        "verify-email".to_string(),
        bson::doc! {
            "code": code.clone(),
            "redirect_url": payload.redirect_url.clone().map(|url| format!("{}?code={}", url, code))
        },
    )
    .await
    {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("failed to send email: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to send email".to_string(),
            });
        }
    };

    HttpResponse::Ok().json(bson::doc! {})
}
