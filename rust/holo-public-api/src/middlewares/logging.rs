use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    middleware::Next,
    web, Error, HttpMessage,
};
use db_utils::schemas::api_log::{ApiLog, LOG_COLLECTION_NAME};
use deadpool_redis::{redis::AsyncCommands, Pool};

use crate::providers::jwt::AccessTokenClaims;

pub async fn logging_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let path = req.path().to_owned();
    let method = req.method().clone();
    let method_str = method.to_string();
    let ip = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_default();

    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_default();

    let authorization = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_default();

    let user_id = req
        .extensions_mut()
        .get::<AccessTokenClaims>()
        .map(|user| user.sub.clone())
        .unwrap_or_default();

    let pool = req.app_data::<web::Data<Pool>>().cloned();

    let response = next.call(req).await;

    let status = match &response {
        Ok(response) => response.status().as_u16(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
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

    let log = ApiLog {
        path: path.clone(),
        method: method_str.clone(),
        ip: ip.clone(),
        user_agent: user_agent.clone(),
        authorization: authorization.clone(),
        user_id: user_id.clone(),
        response_status: status as i32,
        ..Default::default()
    };

    let log_json = match bson::to_document(&log).map_err(|e| {
        tracing::error!("failed to serialize log: {}", e);
        e
    }) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::error!("failed to serialize log: {}", e);
            return response;
        }
    };

    let _: () = conn
        .lpush(LOG_COLLECTION_NAME, log_json.to_string())
        .await
        .unwrap_or_default();

    response
}
