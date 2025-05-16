#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, middleware::from_fn, test::TestRequest, web};

    use crate::{
        controllers::general::health_check,
        middlewares,
        tests::utils::{
            get_app_config, get_cache, perform_integration_test, IntegrationTestResponse, WebData,
        },
    };

    pub async fn build_test_request(web_data: WebData) -> IntegrationTestResponse {
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("user-agent", "integration-test"))
            .uri("/v1/general/health-check");

        let controller = web::scope("")
            .wrap(from_fn(middlewares::limiter::rate_limiter_middleware))
            .service(health_check::health_check);

        perform_integration_test(controller, req, web_data)
            .await
            .unwrap()
    }

    #[actix_web::test]
    pub async fn should_block_requests_after_100_requests() {
        let app_config = get_app_config();
        let cache = get_cache(&app_config).await;

        for _ in 0..100 {
            build_test_request(WebData {
                config: Some(app_config.clone()),
                db: None,
                cache: Some(cache.clone()),
                auth: None,
            })
            .await;
        }

        let resp = build_test_request(WebData {
            config: Some(app_config.clone()),
            db: None,
            cache: Some(cache.clone()),
            auth: None,
        })
        .await;
        assert_eq!(resp.status, StatusCode::TOO_MANY_REQUESTS);
    }
}
