#[cfg(test)]
mod tests {
    use crate::{
        controllers::general::health_check::health_check,
        tests::utils::{perform_integration_test, WebData},
    };
    use actix_web::{http::StatusCode, test};

    #[actix_web::test]
    pub async fn should_succeed_to_health_check() {
        let req = test::TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/general/health-check");

        let resp = perform_integration_test(
            health_check,
            req,
            WebData {
                config: None,
                db: None,
                auth: None,
                cache: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(resp.status, StatusCode::OK);
    }
}
