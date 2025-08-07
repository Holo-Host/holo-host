use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use super::auth_dto::AuthLoginResponse;
use crate::providers::{self, error_response::ErrorResponse};

#[derive(OpenApi)]
#[openapi(
    paths(email_verify_check),
    components(schemas(EmailVerifyCheckRequestDto))
)]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct EmailVerifyCheckRequestDto {
    email: String,
    email_verification_code: String,
}

#[utoipa::path(
    post,
    path = "/public/v1/auth/email-verify-check",
    tag = "Auth",
    summary = "Verify Email Code",
    description = "Check email verification code to confirm if it is valid. Hitting this endpoint does not invalidate the email verification code.",
    request_body = EmailVerifyCheckRequestDto,
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/email-verify-check")]
pub async fn email_verify_check(
    payload: web::Json<EmailVerifyCheckRequestDto>,
    db: web::Data<mongodb::Client>,
    cache: web::Data<deadpool_redis::Pool>,
    req: HttpRequest,
) -> impl Responder {
    match providers::limiter::limiter_by_ip(
        cache,
        req.clone(),
        providers::limiter::LimiterOptions {
            rate_limit_max_requests: 5,
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
    if email_verify.email != payload.email {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "invalid email".to_string(),
        });
    }
    HttpResponse::Ok().json(bson::doc! {})
}
