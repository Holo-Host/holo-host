use actix_limitation::Limiter;

pub fn limit_requests_by_user(
    redis_url: &str,
    amount_of_requests: usize,
    duration_in_seconds: u64
) -> Limiter {
    let limiter = match Limiter::builder(&redis_url.to_string())
        .key_by(|req| {
            let key = req.headers().get("Authorization")
            .and_then(|header| header.to_str().ok())
            .map(|auth_header| auth_header.to_string());
            key.map(|key| format!("limiter/user/{}", key))
        })
        .limit(amount_of_requests)
        .period(std::time::Duration::from_secs(duration_in_seconds))
        .build() {
            Ok(limiter) => limiter,
            Err(err) => {
                tracing::error!("Error setting up limiter: {}", err);
                std::process::exit(1);
            }
        };

    limiter
}

pub fn limit_requests_by_ip(
    redis_url: &str,
    amount_of_requests: usize,
    duration_in_seconds: u64
) -> Limiter {
    let limiter = match Limiter::builder(&redis_url.to_string())
        .key_by(|req| {
            let key =req.peer_addr().map(|addr| addr.to_string());
            key.map(|key| format!("limiter/ip/{}", key))
        })
        .limit(amount_of_requests)
        .period(std::time::Duration::from_secs(duration_in_seconds))
        .build() {
            Ok(limiter) => limiter,
            Err(err) => {
                tracing::error!("Error setting up limiter: {}", err);
                std::process::exit(1);
            }
        };

    limiter
}