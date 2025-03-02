use actix_web::{
    body::{BoxBody, MessageBody}, dev::{
        ServiceRequest,
        ServiceResponse
    }, http::StatusCode, middleware::Next, web, Error, HttpMessage
};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde_json::json;

use crate::providers::jwt::AccessTokenClaims;

pub async fn logging_middleware(
    req: ServiceRequest,
    next: Next<BoxBody>
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let path = req.path().to_owned();
    let method = req.method().clone();
    let method_str = method.to_string();
    let ip = req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_default();

    let user_agent = req.headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_default();

    let authorization = req.headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_default();

    let user_id = req.extensions_mut().get::<AccessTokenClaims>()
        .map(|user| user.sub.clone()).unwrap_or_default();

    let pool = req.app_data::<web::Data<Pool>>().cloned();

    let response = next.call(req).await;

    let status = match &response {
        Ok(response) => response.status().as_u16(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.as_u16()
    };

    if pool.is_none() {
        tracing::error!("Redis pool not found");
        return response;
    }
    let pool = pool.unwrap().get().await;
    if pool.is_err() {
        tracing::error!("failed to connect to redis: {}", pool.err().unwrap());
        return response;
    }
    let mut conn = pool.unwrap();

    let _: () = conn.lpush(
        "logs", json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "path": path,
            "method": method_str,
            "ip": ip,
            "user_agent": user_agent,
            "authorization": authorization,
            "user_id": user_id,
            "status": status,
        }
    ).to_string()).await.unwrap_or_default();

    response
}