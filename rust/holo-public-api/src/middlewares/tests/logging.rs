#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, middleware::from_fn, test::TestRequest, web};
    use db_utils::schemas::api_logs::{ApiLog, LOG_COLLECTION_NAME};
    use deadpool_redis::redis::AsyncCommands;

    use crate::{
        controllers::general::health_check,
        middlewares,
        tests::utils::{
            get_app_config, get_cache, perform_integration_test, IntegrationTestResponse, WebData,
        },
    };

    pub async fn build_test_request(
        req: TestRequest,
        web_data: WebData,
    ) -> IntegrationTestResponse {
        let controller = web::scope("")
            .wrap(from_fn(middlewares::logging::logging_middleware))
            .service(health_check::health_check);

        perform_integration_test(controller, req, web_data)
            .await
            .unwrap()
    }

    #[actix_web::test]
    pub async fn should_push_logs_to_redis_when_api_receives_a_request() {
        let uuid = bson::uuid::Uuid::new();
        let app_config = get_app_config();
        let cache = get_cache(&app_config).await;
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("user-agent", uuid.clone().to_string()))
            .uri("/v1/general/health-check");

        let resp = build_test_request(
            req,
            WebData {
                config: Some(app_config),
                db: None,
                cache: Some(cache.clone()),
                auth: None,
            },
        )
        .await;

        assert_eq!(resp.status, StatusCode::OK);

        let mut conn = cache.get().await.unwrap();

        let logs: Vec<String> = conn
            .lrange(LOG_COLLECTION_NAME.to_string(), 0, 100)
            .await
            .unwrap();
        let mut count = 0;
        for log in logs {
            let log = serde_json::from_str::<ApiLog>(&log).unwrap();
            if log.user_agent == uuid.to_string() {
                assert_eq!(log.path, "/v1/general/health-check");
                assert_eq!(log.method, "GET");
                assert_eq!(log.response_status, 200);
                assert_eq!(log.user_agent, uuid.to_string());
                count += 1;
            }
        }
        assert_eq!(count, 1);
    }
}
