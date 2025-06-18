#[cfg(test)]
mod tests {
    use crate::controllers::general::health_check;
    use crate::middlewares;
    use crate::providers::config::AppConfig;
    use crate::providers::jwt::{sign_access_token, AccessTokenClaims};
    use crate::tests::utils::{
        get_app_config, perform_integration_test, IntegrationTestResponse, WebData,
    };
    use actix_web::http::StatusCode;
    use actix_web::middleware::from_fn;
    use actix_web::test::TestRequest;
    use actix_web::web;

    pub async fn build_test_request(
        app_config: AppConfig,
        req: TestRequest,
    ) -> IntegrationTestResponse {
        let controller = web::scope("")
            .wrap(from_fn(middlewares::auth::auth_middleware))
            .service(health_check::health_check);

        perform_integration_test(
            controller,
            req,
            WebData {
                config: Some(app_config),
                db: None,
                cache: None,
                auth: None,
            },
        )
        .await
        .unwrap()
    }

    #[actix_web::test]
    pub async fn should_successfully_authenticate_user() {
        let app_config = get_app_config();
        let token = sign_access_token(
            AccessTokenClaims {
                sub: bson::oid::ObjectId::new().to_string(),
                exp: 1000000000000,
                permissions: vec![],
            },
            app_config.jwt_secret.as_ref(),
        )
        .unwrap();

        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .uri("/v1/general/health-check");

        let resp = build_test_request(app_config, req).await;

        print!("{:?}", resp.status.clone());
        print!("{:?}", resp.body.clone());
        assert_eq!(resp.status, StatusCode::OK);
    }

    #[actix_web::test]
    pub async fn should_fail_to_authenticate_user_with_no_authorization_header() {
        let app_config = get_app_config();
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/general/health-check");

        let resp = build_test_request(app_config, req).await;

        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    pub async fn should_fail_to_authenticate_user_with_invalid_token() {
        let app_config = get_app_config();
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", "Bearer invalid_token"))
            .uri("/v1/general/health-check");

        let resp = build_test_request(app_config, req).await;

        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    pub async fn should_fail_to_authenticate_user_with_expired_token() {
        let app_config = get_app_config();
        let token = sign_access_token(
            AccessTokenClaims {
                sub: bson::oid::ObjectId::new().to_string(),
                exp: 0,
                permissions: vec![],
            },
            app_config.jwt_secret.as_ref(),
        )
        .unwrap();

        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .uri("/v1/general/health-check");

        let resp = build_test_request(app_config, req).await;

        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    pub async fn should_fail_to_authenticate_user_with_invalid_user_id() {
        let app_config = get_app_config();
        let token = sign_access_token(
            AccessTokenClaims {
                sub: "invalid_user_id".to_string(),
                exp: 1000000000000,
                permissions: vec![],
            },
            app_config.jwt_secret.as_ref(),
        )
        .unwrap();

        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .uri("/v1/general/health-check");

        let resp = build_test_request(app_config, req).await;

        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }
}
