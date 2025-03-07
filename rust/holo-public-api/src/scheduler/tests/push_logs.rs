#[cfg(test)]
mod tests {
    use bson::uuid;
    use deadpool_redis::redis::AsyncCommands;

    use crate::{
        providers::database::{cursor_to_vec, schemas::{
            log::{
                Log, LOG_COLLECTION_NAME
            },
            shared::meta::new_meta
        }}, scheduler::push_logs, tests::utils::{get_app_config, get_cache, get_db}
    };

    #[tokio::test]
    async fn should_push_logs_to_mongodb_when_there_are_logs_in_redis() {
        let app_config = get_app_config();
        let mongodb = get_db(&app_config).await;
        let cache = get_cache(&app_config).await;

        let mut conn = cache.get().await.unwrap();
    
        let log = Log {
            oid: None,
            meta: new_meta(),
            id: uuid::Uuid::new().to_string(),
            timestamp: bson::DateTime::now(),
            path: "/".to_string(),
            method: "GET".to_string(),
            ip: "127.0.0.1".to_string(),
            user_agent: "test".to_string(),
            authorization: "test".to_string(),
            user_id: "1".to_string(),
            status: 200,
        };
        let log_json = serde_json::to_string(&log).unwrap();

        let _: () = conn.lpush(LOG_COLLECTION_NAME, log_json).await.unwrap();

        let mut now = chrono::Utc::now();
        push_logs::push_logs(now, cache.clone(), mongodb.clone()).await.unwrap();
        now = now + chrono::Duration::seconds(30);
        push_logs::push_logs(now, cache.clone(), mongodb.clone()).await.unwrap();

        let collection = mongodb.collection::<Log>(LOG_COLLECTION_NAME);
        let cursor = collection.aggregate(
            vec![
                bson::doc!{
                    "$match": {
                        "id": log.id
                    }
                }
            ], None).await.unwrap();
        let logs = cursor_to_vec::<Log>(cursor).await.unwrap();
        assert_eq!(logs.len(), 1);
        let log = logs[0].clone();
        assert_eq!(log.id, log.id);
    }
}