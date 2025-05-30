#[cfg(test)]
mod tests {
    use crate::{
        controllers::auth::{auth_dto::AuthLoginResponse, login_apikey::login_with_apikey},
        providers::{auth::API_KEY_HEADER, crud},
        tests::utils,
    };
    use actix_web::{http::StatusCode, test};
    use bson::oid::ObjectId;
    use db_utils::schemas::api_key::{ApiKey, API_KEY_COLLECTION_NAME};

    #[actix_web::test]
    pub async fn should_successfully_login_with_apikey() {
        let config = utils::get_app_config();
        let db = utils::get_db(config.clone()).await;
        let owner_id = ObjectId::new();
        let api_key = bson::uuid::Uuid::new().to_string().replace("-", "");
        crud::create::<ApiKey>(
            db.clone(),
            API_KEY_COLLECTION_NAME.to_string(),
            ApiKey {
                _id: None,
                api_key: api_key.clone(),
                description: "test-api-key".to_string(),
                expire_at: 10,
                permissions: vec![],
                metadata: db_utils::schemas::metadata::Metadata::default(),
                owner: owner_id,
            },
        )
        .await
        .unwrap();

        let req = test::TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .insert_header((API_KEY_HEADER.to_string(), format!("v0-{}", api_key)))
            .uri("/v1/auth/login-with-apikey");
        let resp = utils::perform_integration_test(
            login_with_apikey,
            req,
            utils::WebData {
                config: Some(config.clone()),
                auth: None,
                cache: Some(utils::get_cache(config.clone()).await),
                db: Some(db.clone()),
            },
        )
        .await
        .unwrap();

        assert_eq!(resp.status, StatusCode::OK);
        let body: AuthLoginResponse = bson::from_document(resp.body.unwrap()).unwrap();
        assert!(
            !body.access_token.is_empty(),
            "access_token should not be empty"
        );
        assert!(
            !body.refresh_token.is_empty(),
            "refresh_token should not be empty"
        );
    }
}
