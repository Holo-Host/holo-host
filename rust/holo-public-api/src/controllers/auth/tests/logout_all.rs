#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test::TestRequest};

    use crate::{
        controllers::auth::logout_all::logout_all, providers::{database::schemas::{shared::meta::new_meta, user::{User, USER_COLLECTION_NAME}}, jwt::AccessTokenClaims}, tests::utils::{
            get_app_config,
            get_db,
            perform_integration_test,
            WebData
        }
    };

    #[actix_web::test]
    pub async fn should_succeed_to_logout_all_sessions_by_user_id() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;
    
        let result = db.collection::<User>(
            USER_COLLECTION_NAME
        ).insert_one(
            User {
                _id: None,
                _meta: new_meta(),
                refresh_token_version: 0,
                permissions: vec![],
                roles: vec![]
            },
            None
        ).await.unwrap();
        let user_id = result.inserted_id.as_object_id().unwrap();
        
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/auth/logout-all");

        let resp = perform_integration_test(
            logout_all,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 0,
                    permissions: vec![],
                })
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::OK);

        let user = db.collection::<User>(
            USER_COLLECTION_NAME
        ).find_one(
            bson::doc!{ "_id": user_id },
            None
        ).await.unwrap();
        assert_eq!(user.unwrap().refresh_token_version, 1);
    }
}