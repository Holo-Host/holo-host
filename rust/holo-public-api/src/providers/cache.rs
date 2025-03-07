use deadpool_redis::*;

pub async fn setup_cache(
    redis_url: &str
) -> Result<Pool, anyhow::Error> {
    let pool = Config::from_url(redis_url)
        .create_pool(Some(Runtime::Tokio1))
        .expect("failed to create redis pool");

    Ok(pool)
}