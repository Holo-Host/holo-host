#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test};
    use bson::oid::ObjectId;
    use db_utils::schemas::{
        api_key::{ApiKey, API_KEY_COLLECTION_NAME},
        user_permissions::{PermissionAction, UserPermission},
    };

    use crate::{
        controllers::apikey::create_apikey::{
            create_api_key, CreateApiKeyRequest, CreateApiKeyResponse,
        },
        providers::crud,
        providers::jwt::AccessTokenClaims,
        tests::utils,
    };

    #[actix_web::test]
    pub async fn should_successfully_create_apikey() {
        let config = utils::get_app_config();
        let db = utils::get_db(config.clone()).await;
        let req = test::TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .set_json(CreateApiKeyRequest {
                description: "test-api-key".to_string(),
                version: "v0".to_string(),
                permissions: vec![],
                expire_at: 10,
            })
            .uri("/v1/apikey");

        let resp = utils::perform_integration_test(
            create_api_key,
            req,
            utils::WebData {
                config: Some(config.clone()),
                auth: Some(AccessTokenClaims {
                    sub: ObjectId::new().to_hex(),
                    permissions: vec![UserPermission {
                        resource: API_KEY_COLLECTION_NAME.to_string(),
                        action: PermissionAction::Create,
                        owner: "self".to_string(),
                    }],
                    exp: (bson::DateTime::now().to_chrono().timestamp() + 60) as usize,
                    initials: None,
                }),
                cache: Some(utils::get_cache(config.clone()).await),
                db: Some(db.clone()),
            },
        )
        .await
        .unwrap();

        assert_eq!(resp.status, StatusCode::OK);
        let body: CreateApiKeyResponse = bson::from_document(resp.body.unwrap()).unwrap();
        let db_entry =
            crud::get::<ApiKey>(db.clone(), API_KEY_COLLECTION_NAME.to_string(), body.id)
                .await
                .unwrap();
        assert!(db_entry.is_some());
    }
}
