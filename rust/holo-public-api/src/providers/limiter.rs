use actix_web::web;
use deadpool_redis::redis::AsyncCommands;

pub struct LimiterOptions {
    /// number of requests allowed per window
    pub rate_limit_max_requests: u32,
    /// time window in seconds
    pub rate_limit_window: u32,
}

/// endpoint limiter, This gives more fine grained control over rate limiting specific endpoints
/// this can be used to rate limit using a specific key
pub async fn limiter_by_key(
    cache: web::Data<deadpool_redis::Pool>,
    key: String,
    options: LimiterOptions,
) -> bool {
    let mut conn = cache.get().await.unwrap();
    let count: u32 = conn.get(key.clone()).await.unwrap_or(0);
    if count >= options.rate_limit_max_requests {
        return false;
    }

    match conn
        .set_ex::<_, _, ()>(key.clone(), count + 1, options.rate_limit_window as u64)
        .await
    {
        Ok(_) => true,
        Err(error) => {
            tracing::error!("Failed to set rate limit: {}", error);
            false
        }
    }
}

/// endpoint limiter, This gives more fine grained control over rate limiting specific endpoints
/// this can be used to rate limit by ip address
pub async fn limiter_by_ip(
    cache: web::Data<deadpool_redis::Pool>,
    req: actix_web::HttpRequest,
    options: LimiterOptions,
) -> bool {
    let ip = req
        .peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_default();
    let url = req.uri().to_string();
    let key = format!("rate-limit:{}-{}", url, ip);

    limiter_by_key(cache, key, options).await
}
