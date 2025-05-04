use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use bson::doc;
use utoipa::OpenApi;

use super::auth_dto::AuthLoginResponse;
use crate::providers::{auth, config::AppConfig, error_response::ErrorResponse};

#[derive(OpenApi)]
#[openapi(paths(login_with_apikey))]
pub struct OpenApiSpec;

#[utoipa::path(
    get,
    path = "/public/v1/auth/login-with-apikey",
    tag = "Auth",
    summary = "Login with API key",
    description = "Authenticate user with API key",
    params(
      ("x-api-key", Header, description = "API key to authenticate user", example = "v0-1234567890abcdef12345678"),
    ),
    responses(
        (status = 200, body = AuthLoginResponse)
    )
)]
#[get("/v1/auth/login-with-apikey")]
pub async fn login_with_apikey(
    req: HttpRequest,
    config: web::Data<AppConfig>,
    db: web::Data<mongodb::Client>,
) -> impl Responder {
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
    let api_key_hash = auth::get_apikey_hash(api_key[0].to_string(), api_key[1].to_string());

    // get user id and permissions from the api key hash
    let result =
        match auth::get_user_id_and_permissions_from_apikey(db.get_ref(), api_key_hash.unwrap())
            .await
        {
            Ok(result) => result,
            Err(_err) => {
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "failed to get user id and permissions".to_string(),
                });
            }
        };
    if result.is_none() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "missing or invalid 'api-key'".to_string(),
        });
    }
    let result = result.unwrap();
    let user_id = result.owner.to_string();
    let version = auth::get_refresh_token_version(db.get_ref(), user_id.clone()).await;
    let permissions = result.permissions.clone();
    let jwt_tokens = auth::sign_jwt_tokens(
        &config.get_ref().jwt_secret,
        user_id.clone(),
        permissions.clone(),
        version,
        false,
        result.expire_at as usize,
        Some(result._id.unwrap().to_string()),
    );
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
