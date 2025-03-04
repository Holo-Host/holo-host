use std::{num::NonZero, str::FromStr, thread};

use cron::Schedule;
use deadpool_redis::{redis::AsyncCommands, Pool};
use mongodb::Database;

use crate::providers::database::schemas::log::{Log, LOG_COLLECTION_NAME};

pub async fn push_logs(
    now: chrono::DateTime<chrono::Utc>,
    cache: Pool,
    mongodb: Database
) -> Result<(), anyhow::Error> {
    // every 30 seconds
    let schedule = match Schedule::from_str("0/30 * * * * *") {
        Ok(schedule) => schedule,
        Err(err) => {
            tracing::error!("Error parsing cron expression: {}", err);
            return Ok(());
        }
    };

    for dt in schedule.upcoming(chrono::Utc).take(1) {
        let until = dt - now;
        thread::sleep(until.to_std().unwrap());

        tracing::info!("Pushing logs to MongoDB");
        let mut conn = cache.get().await?;
        let len: usize = conn.llen(LOG_COLLECTION_NAME).await?;
        if len == 0 {
            return Ok(());
        }
        let logs_json: Vec<String> = conn.lpop(LOG_COLLECTION_NAME, Some(NonZero::new(len).unwrap())).await?;
        let logs: Vec<Log> = logs_json.iter().map(|log_json| serde_json::from_str(log_json)).collect::<Result<Vec<Log>, serde_json::Error>>()?;

        let collection = mongodb.collection::<Log>(LOG_COLLECTION_NAME);
        if !logs.is_empty() {
            collection.insert_many(logs, None).await?;
        }
    }

    Ok(())
}