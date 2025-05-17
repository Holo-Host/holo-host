use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    middleware::Next,
    web, Error,
};
use deadpool_redis::{redis::AsyncCommands, Pool};

use crate::providers::{config::AppConfig, error_response::create_middleware_error_response};

/// middleware to add a global rate limiter on every request
pub async fn rate_limiter_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let ip = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_default();

    let authorization = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_owned());

    let app_config = match req.app_data::<web::Data<AppConfig>>().cloned() {
        Some(app_config) => app_config,
        None => {
            tracing::error!("No app config found");
            return create_middleware_error_response(
                req,
                StatusCode::INTERNAL_SERVER_ERROR,
                "No app config found",
            );
        }
    };

    let pool = match req.app_data::<web::Data<Pool>>().cloned() {
        Some(pool) => pool,
        None => {
            tracing::error!("No redis pool found");
            return create_middleware_error_response(
                req,
                StatusCode::INTERNAL_SERVER_ERROR,
                "No redis pool found",
            );
        }
    };

    let conn = match pool.get().await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::error!("Failed to connect to redis: {}", err);
            return create_middleware_error_response(
                req,
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to connect to redis",
            );
        }
    };
    let mut conn = conn;

    let limit = app_config.rate_limit_max_requests.unwrap_or(100);
    let window = app_config.rate_limit_window.unwrap_or(60);
    let mut keys = vec![format!("rate_limit:{}", ip)];
    if authorization.is_some() {
        keys.push(format!("rate_limit:{}", authorization.unwrap()));
    }
    for key in keys {
        let count: u32 = conn.get(&key).await.unwrap_or(0);
        if count >= limit {
            return create_middleware_error_response(
                req,
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded",
            );
        }

        conn.set_ex(key, count + 1, window as u64)
            .await
            .unwrap_or(());
    }
    let resp = next.call(req).await?;
    Ok(resp.map_into_boxed_body())
}
