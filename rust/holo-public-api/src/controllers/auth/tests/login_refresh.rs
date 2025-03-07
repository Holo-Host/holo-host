#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test::TestRequest};
    use serde_json::json;

    use crate::{
        controllers::auth::login_refresh::login_refresh,
        providers::database::schemas::{
            shared::meta::new_meta,
            user::{User, USER_COLLECTION_NAME}
        },
        tests::utils::{
            create_credentials, get_app_config, get_db, perform_integration_test, WebData
        }
    };

    #[actix_web::test]
    pub async fn should_succeed_to_refresh_token_when_access_token_is_expired() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let result = db.collection::<User>(
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

        let (access_token, refresh_token) = create_credentials(
            &app_config.jwt_secret, user_id
        );

        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/login-refresh")
            .set_json(json!({ "access_token": access_token, "refresh_token": refresh_token }));

        let resp = perform_integration_test(
            login_refresh,
            req,
            WebData { config: Some(app_config), db: Some(db), auth: None, cache: None },
        ).await.unwrap();

        let body = resp.body.unwrap();
        println!("body: {:?}", body.clone());
        assert_eq!(resp.status, StatusCode::OK);
        let access_token = body.get("access_token").unwrap().as_str().unwrap();
        let refresh_token = body.get("refresh_token").unwrap().as_str().unwrap();
        assert!(!access_token.is_empty());
        assert!(!refresh_token.is_empty());
    }

    #[actix_web::test]
    pub async fn should_fail_to_refresh_token_when_refresh_token_is_invalid() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let result = db.collection::<User>(
            USER_COLLECTION_NAME
        ).insert_one(
            User {
                oid: None,
                permissions: vec![],
                meta: new_meta(),
                refresh_token_version: 0,
                roles: vec![],
            },
            None
        ).await.unwrap();
        let user_id = result.inserted_id.as_object_id().unwrap();
        
        let (access_token, _) = create_credentials(
            &app_config.jwt_secret, user_id
        );

        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/login-refresh")
            .set_json(json!({ "access_token": access_token, "refresh_token": "invalid" }));

        let resp = perform_integration_test(
            login_refresh,
            req,
            WebData { config: Some(app_config), db: Some(db), auth: None, cache: None },
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    pub async fn should_fail_to_refresh_token_when_access_token_is_invalid() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let result = db.collection::<User>(
            USER_COLLECTION_NAME
        ).insert_one(
            User {
                oid: None,
                permissions: vec![],
                meta: new_meta(),
                refresh_token_version: 0,
                roles: vec![],
            },
            None
        ).await.unwrap();
        let user_id = result.inserted_id.as_object_id().unwrap();
        
        let (_, refresh_token) = create_credentials(
            &app_config.jwt_secret, user_id
        );

        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/login-refresh")
            .set_json(json!({ "access_token": "invalid", "refresh_token": refresh_token }));

        let resp = perform_integration_test(
            login_refresh,
            req,
            WebData { config: Some(app_config), db: Some(db), auth: None, cache: None },
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    pub async fn should_fail_to_refresh_token_when_user_does_not_exist() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

            let user_id = bson::oid::ObjectId::new();
        let (access_token, refresh_token) = create_credentials(&app_config.jwt_secret, user_id);

        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/login-refresh")
            .set_json(json!({ "access_token": access_token, "refresh_token": refresh_token }));

        let resp = perform_integration_test(
            login_refresh,
            req,
            WebData { config: Some(app_config), db: Some(db), auth: None, cache: None },
        ).await.unwrap();

        let body = resp.body.unwrap();
        println!("body: {:?}", body.clone());
        assert_eq!(resp.status, StatusCode::UNAUTHORIZED);
    }
}
