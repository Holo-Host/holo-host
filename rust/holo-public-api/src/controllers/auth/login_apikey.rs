use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use utoipa::OpenApi;

use super::auth_dto::AuthLoginResponse;
use crate::providers::{
    self, auth,
    config::AppConfig,
    error_response::ErrorResponse,
    jwt::{AccessTokenClaims, RefreshTokenClaims},
};

#[derive(OpenApi)]
#[openapi(paths(login_with_apikey))]
pub struct OpenApiSpec;

#[utoipa::path(
    post,
    path = "/public/v1/auth/login-with-apikey",
    tag = "Auth",
    summary = "Login with API key",
    description = "Use an api key to login and get an access token + refresh token. Rate limit: 3 requests per minute.",
    params(
      ("x-api-key", Header, description = "API key to authenticate user", example = "v0-1234567890abcdef12345678"),
    ),
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[post("/v1/auth/login-with-apikey")]
pub async fn login_with_apikey(
    req: HttpRequest,
    config: web::Data<AppConfig>,
    db: web::Data<mongodb::Client>,
    cache: web::Data<deadpool_redis::Pool>,
) -> impl Responder {
    match providers::limiter::limiter_by_ip(
        cache,
        req.clone(),
        providers::limiter::LimiterOptions {
            rate_limit_max_requests: 3,
            rate_limit_window: 60,
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

    let api_key = auth::get_apikey_from_headers(&req);
    if api_key.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "missing or invalid 'api-key'".to_string(),
        });
    }
    // get api key hash depending on the api key version
    let api_key = api_key.unwrap();
    let api_key: Vec<String> = api_key.split("-").map(|s| s.to_string()).collect();
    if api_key.len() != 2 {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "missing or invalid 'api-key'".to_string(),
        });
    }
    let result = auth::compare_and_fetch_apikey(
        api_key[0].to_string(),
        api_key[1].to_string(),
        db.get_ref().clone(),
    )
    .await;
    if result.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "missing or invalid 'api-key'".to_string(),
        });
    }
    let result = result.unwrap();
    let user_id = result.owner.to_string();
    let version = auth::get_refresh_token_version(db.get_ref().clone(), user_id.clone()).await;
    let permissions = result.permissions.clone();
    let jwt_tokens = auth::sign_tokens(auth::SignJwtTokenOptions {
        jwt_secret: config.jwt_secret.clone(),
        access_token: AccessTokenClaims {
            sub: user_id.clone(),
            permissions: permissions.clone(),
            exp: bson::DateTime::now().to_chrono().timestamp() as usize
                + config.access_token_expiry.unwrap_or(300) as usize,
        },
        refresh_token: RefreshTokenClaims {
            version,
            sub: user_id.clone(),
            exp: result.expire_at as usize,
            allow_extending_refresh_token: false,
            reference_id: Some(result._id.unwrap().to_string()),
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
