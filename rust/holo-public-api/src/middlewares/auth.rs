use crate::providers::{self, error_response::create_middleware_error_response};
use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    middleware::Next,
    web, Error, HttpMessage,
};

pub async fn auth_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let auth_header = req.headers().get("authorization");
    if auth_header.is_none() {
        return create_middleware_error_response(
            req,
            StatusCode::UNAUTHORIZED,
            "No authorization header",
        );
    }

    let auth_header = auth_header.unwrap();
    let auth_header = auth_header.to_str().unwrap();

    // get access token from authorization header (Bearer <token>)
    let token = auth_header.split(" ").nth(1).unwrap_or_default();

    // get jwt secret from app config
    let config = match req.app_data::<web::Data<providers::config::AppConfig>>() {
        Some(config) => config,
        None => {
            return create_middleware_error_response(req, StatusCode::UNAUTHORIZED, "No app config")
        }
    };

    // verify access token
    let claims = match providers::jwt::verify_access_token(token, &config.jwt_secret) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::debug!("Error verifying token: {}", err);
            return create_middleware_error_response(
                req,
                StatusCode::UNAUTHORIZED,
                "Invalid token",
            );
        }
    };

    // check if access token is expired
    let now = bson::DateTime::now().to_chrono().timestamp() as usize;
    if claims.exp < now {
        return create_middleware_error_response(req, StatusCode::UNAUTHORIZED, "Token expired");
    }

    // verify user id is valid
    match bson::oid::ObjectId::parse_str(claims.sub.clone()) {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("Invalid user id with a valid token: {}", err);
            return create_middleware_error_response(
                req,
                StatusCode::UNAUTHORIZED,
                "Invalid user id",
            );
        }
    }

    // insert claims as app_data for this request
    req.extensions_mut().insert(claims);

    let resp = next.call(req).await?;
    Ok(resp.map_into_boxed_body())
}
