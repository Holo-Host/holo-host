use crate::providers::{self, error_response::ErrorResponse};
use actix_web::{
    body::{BoxBody, MessageBody}, dev::{
        ServiceRequest,
        ServiceResponse
    }, middleware::Next, web, Error, HttpMessage, HttpResponse
};

pub fn build_unauthorized_response(req: ServiceRequest, message: &str)
-> Result<ServiceResponse<BoxBody>, Error> {
    let (req_http, _) = req.into_parts();
    let resp: HttpResponse<BoxBody> = HttpResponse::Unauthorized().json(ErrorResponse {
        message: message.to_string(),
    });

    Ok(ServiceResponse::new(req_http, resp))
}

pub async fn auth_middleware(
    req: ServiceRequest,
    next: Next<BoxBody>
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let auth_header = req.headers().get("authorization");
    if auth_header.is_none() {
        return build_unauthorized_response(req, "No authorization header");
    }

    let auth_header = auth_header.unwrap();
    let auth_header = auth_header.to_str().unwrap();

    // get access token from authorization header (Bearer <token>)
    let token = auth_header.split(" ").nth(1).unwrap_or_default();

    // get jwt secret from app config
    let config = match req.app_data::<web::Data<providers::config::AppConfig>>() {
        Some(config) => config,
        None => return build_unauthorized_response(req, "No app config")
    };

    // verify access token
    let claims = match providers::jwt::verify_access_token(token, &config.jwt_secret) {
        Ok(claims) => claims,
        Err(err) => {
            tracing::debug!("Error verifying token: {}", err);
            return build_unauthorized_response(req, "Invalid token");
        }
    };

    // check if access token is expired
    if claims.exp < chrono::Utc::now().timestamp() as usize {
        return build_unauthorized_response(req, "Token expired");
    }

    // verify user id is valid
    match bson::oid::ObjectId::parse_str(claims.sub.clone()) {
        Ok(_) => {}
        Err(err) => {
            tracing::error!("Invalid user id with a valid token: {}", err);
            return build_unauthorized_response(req, "Invalid user id");
        }
    }

    // insert claims as app_data for this request
    req.extensions_mut().insert(claims);

    next.call(req).await
}