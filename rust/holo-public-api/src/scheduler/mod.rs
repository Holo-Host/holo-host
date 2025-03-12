use deadpool_redis::Pool;
use mongodb::Database;
use crate::providers::config::AppConfig;

mod push_logs;
mod tests;

pub async fn setup_scheduler(
    _: AppConfig,
    mongodb: Database,
    cache: Pool
) -> Result<(), anyhow::Error> {
    tokio::spawn(async move {
        tokio::spawn(push_logs::push_logs_cronjob(cache.clone(), mongodb.clone()));
    });

    Ok(())
}