#[cfg(test)]
mod tests {
    use crate::{
        controllers::apikey::{apikey_dto::ApiKeyDto, get_apikey::get_apikey},
        providers::{crud, jwt::AccessTokenClaims},
        tests::utils,
    };
    use actix_web::{http::StatusCode, test};
    use bson::oid::ObjectId;
    use db_utils::schemas::{
        api_key::{ApiKey, API_KEY_COLLECTION_NAME},
        user_permissions::{PermissionAction, UserPermission},
    };

    #[actix_web::test]
    pub async fn should_successfully_get_apikey() {
        let config = utils::get_app_config();
        let db = utils::get_db(config.clone()).await;
        let owner_id = ObjectId::new();
        let api_key_id = crud::create::<ApiKey>(
            db.clone(),
            API_KEY_COLLECTION_NAME.to_string(),
            ApiKey {
                _id: None,
                api_key: bson::uuid::Uuid::new().to_string(),
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
            .uri(&format!("/v1/apikey/{}", api_key_id.to_hex()));

        let resp = utils::perform_integration_test(
            get_apikey,
            req,
            utils::WebData {
                config: Some(config.clone()),
                auth: Some(AccessTokenClaims {
                    sub: owner_id.to_hex(),
                    permissions: vec![UserPermission {
                        resource: API_KEY_COLLECTION_NAME.to_string(),
                        action: PermissionAction::Read,
                        owner: "self".to_string(),
                    }],
                    exp: (bson::DateTime::now().to_chrono().timestamp() + 60) as usize,
                }),
                cache: Some(utils::get_cache(config.clone()).await),
                db: Some(db.clone()),
            },
        )
        .await
        .unwrap();

        assert_eq!(resp.status, StatusCode::OK);
        let body: ApiKeyDto = bson::from_document(resp.body.unwrap()).unwrap();
        assert_eq!(body.id, api_key_id.to_hex());
    }
}
