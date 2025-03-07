#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;
    use actix_web::test::TestRequest;
    use serde_json::json;

    use crate::providers::database::schemas::shared::meta::new_meta;
    use crate::providers::database::schemas::user::{User, USER_COLLECTION_NAME};
    use crate::providers::database::schemas::api_key::{generate_api_key, ApiKey, API_KEY_COLLECTION_NAME};
    use crate::controllers::auth::login_with_api_key::login_with_api_key;
    use crate::tests::utils::{
        get_app_config,
        get_db,
        perform_integration_test,
        WebData
    };

    #[actix_web::test]
    async fn should_login_successfully() {
        let api_key = generate_api_key();

        let app_config = get_app_config();
        let db = get_db(&app_config).await;
        
        let result =db.collection::<User>(
            USER_COLLECTION_NAME
        ).insert_one(
            User {
                oid: None,
                permissions: vec![],
                meta: new_meta(),
                refresh_token_version: 0,
                roles: vec![]
            },
            None
        ).await.unwrap();
        let user_id = result.inserted_id.as_object_id().unwrap();

        db.collection::<ApiKey>(
            API_KEY_COLLECTION_NAME
        ).insert_one(
            ApiKey {
                oid: None,
                meta: new_meta(),
                user_id: user_id,
                key: api_key.clone(),
            },
            None
        ).await.unwrap();

        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/login-with-api-key")
            .set_json(json!({ "api_key": api_key }));

        let resp = perform_integration_test(
            login_with_api_key,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
                auth: None,
                cache: None
            },
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::OK);
        let body = resp.body.unwrap();
        let access_token = body.get("access_token").unwrap().as_str().unwrap();
        let refresh_token = body.get("refresh_token").unwrap().as_str().unwrap();
        assert!(!access_token.is_empty());
        assert!(!refresh_token.is_empty());
    }

    #[actix_web::test]
    async fn should_fail_to_login_when_api_key_is_invalid() {

        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/login-with-api-key")
            .set_json(json!({ "api_key": "invalid" }));

        let resp = perform_integration_test(
            login_with_api_key,
            req,
            WebData { config: Some(app_config), db: Some(db), auth: None, cache: None },
        ).await.unwrap();
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }
}