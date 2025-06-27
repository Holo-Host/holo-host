use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use db_utils::schemas::{
    user::{self, User, USER_COLLECTION_NAME},
    user_info::{self, UserInfo, USER_INFO_COLLECTION_NAME},
    user_password::{UserPassword, USER_PASSWORD_COLLECTION_NAME},
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use super::auth_dto::AuthLoginResponse;
use crate::providers::{
    self, auth,
    config::AppConfig,
    crud,
    error_response::ErrorResponse,
    jwt::{AccessTokenClaims, RefreshTokenClaims},
};

#[derive(OpenApi)]
#[openapi(
    paths(login_with_password),
    components(schemas(LoginWithPasswordRequestDto))
)]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct LoginWithPasswordRequestDto {
    #[schema(example = "john.smith@example.com")]
    email: String,
    #[schema(example = "123456")]
    password: String,
}

#[utoipa::path(
    post,
    path = "/public/v1/auth/login-with-password",
    tag = "Auth",
    summary = "Login with password",
    description = "Use email and password to login",
    request_body = LoginWithPasswordRequestDto,
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/login-with-password")]
pub async fn login_with_password(
    req: HttpRequest,
    payload: web::Json<LoginWithPasswordRequestDto>,
    config: web::Data<AppConfig>,
    db: web::Data<mongodb::Client>,
    cache: web::Data<deadpool_redis::Pool>,
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

    let user_info = match crud::find_one::<UserInfo>(
        db.get_ref().clone(),
        USER_INFO_COLLECTION_NAME.to_string(),
        bson::doc! {
           "email": payload.email.clone(),
        },
    )
    .await
    {
        Ok(user_info) => user_info,
        Err(err) => {
            tracing::error!("failed to get user info: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to get user info".to_string(),
            });
        }
    };
    if user_info.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "invalid email or password".to_string(),
        });
    }
    let user_info = user_info.unwrap();
    let user_id = user_info.user_id.clone();

    let user_password = match crud::find_one::<UserPassword>(
        db.get_ref().clone(),
        USER_PASSWORD_COLLECTION_NAME.to_string(),
        bson::doc! {
            "owner": user_id,
        },
    )
    .await
    {
        Ok(user_password) => user_password,
        Err(err) => {
            tracing::error!("failed to get user password: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to get user password".to_string(),
            });
        }
    };
    if user_password.is_none() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "invalid email or password".to_string(),
        });
    }
    match bcrypt::verify(&payload.password, &user_password.unwrap().password_hash) {
        Ok(result) => match result {
            true => {}
            false => {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    message: "invalid email or password".to_string(),
                });
            }
        },
        Err(_) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "invalid email or password".to_string(),
            });
        }
    }

    let user = match crud::find_one::<User>(
        db.get_ref().clone(),
        USER_COLLECTION_NAME.to_string(),
        bson::doc! {
            "_id": user_id
        },
    )
    .await
    {
        Ok(user_password) => user_password,
        Err(err) => {
            tracing::error!("failed to get user password: {}", err);
            return HttpResponse::InternalServerError().json(bson::doc! {
                "error": err.to_string(),
                "message": "failed to get user password".to_string(),
            });
        }
    };
    if user.is_none() {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "invalid user".to_string(),
        });
    }
    let user = user.unwrap();
    let user_id = user._id.unwrap().to_hex();
    let day_in_seconds = 24 * 60 * 60;

    let given_name_last_char = user_info.given_names.chars().take(1).collect::<String>();
    let family_name_last_char = user_info.family_name.chars().take(1).collect::<String>();
    let jwt_tokens = auth::sign_tokens(auth::SignJwtTokenOptions {
        jwt_secret: config.jwt_secret.clone(),
        access_token: AccessTokenClaims {
            sub: user_id.clone(),
            permissions: user.permissions,
            exp: bson::DateTime::now().to_chrono().timestamp() as usize
                + config.access_token_expiry.unwrap_or(300) as usize,
            initials: Some(format!("{}{}", given_name_last_char, family_name_last_char)),
        },
        refresh_token: RefreshTokenClaims {
            version: user.refresh_token_version,
            sub: user_id.clone(),
            exp: day_in_seconds * 30,
            allow_extending_refresh_token: false,
            reference_id: None,
        },
    });
    if jwt_tokens.is_none() {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "failed to sign jwt tokens".to_string(),
        });
    }
    let (access_token, refresh_token) = jwt_tokens.unwrap();

    HttpResponse::Ok().json(AuthLoginResponse {
        access_token,
        refresh_token,
    })
}
