use std::time::Duration;

use chrono::Utc;
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
        loop {
            let now = Utc::now();
            tokio::spawn(push_logs::push_logs(now, cache.clone(), mongodb.clone()));
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    Ok(())
}