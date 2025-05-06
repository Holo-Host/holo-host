use crate::providers::config::AppConfig;
use deadpool_redis::Pool;

pub async fn setup_scheduler(
    _: AppConfig,
    _mongodb: mongodb::Client,
    _cache: Pool,
) -> Result<(), anyhow::Error> {
    Ok(())
}
