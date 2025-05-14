use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    middleware::Next,
    web, Error,
};
use deadpool_redis::{redis::AsyncCommands, Pool};

use crate::providers::error_response::create_middleware_error_response;

pub async fn logging_middleware(
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
        .map(|s| s.to_owned())
        .unwrap_or_default();

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

    let keys = vec![
        format!("rate_limit:{}", ip),
        format!("rate_limit:{}", authorization),
    ];
    for key in keys {
        let count: u32 = conn.get(&key).await.unwrap_or(0);

        let limit = 100; // requests
        let window = 60; // seconds

        if count >= limit {
            return create_middleware_error_response(
                req,
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded",
            );
        }

        let _: () = if count == 0 {
            let mut pipe = deadpool_redis::redis::pipe();
            pipe.cmd("SET").arg(&key).arg(1).ignore();
            pipe.cmd("EXPIRE").arg(&key).arg(window).ignore();
            pipe.query_async(&mut conn).await.unwrap_or(())
        } else {
            conn.incr(&key, 1).await.unwrap_or(())
        };
    }
    let resp = next.call(req).await?;
    Ok(resp.map_into_boxed_body())
}
