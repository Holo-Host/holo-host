#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test::TestRequest};

    use crate::{
        controllers::workloads::create_workload::{self, CreateWorkloadRequest},
        providers::{database::schemas::shared::system_specs, jwt::AccessTokenClaims, permissions::WORKLOADS_CREATE},
        tests::utils::{
            get_app_config, get_db,
            perform_integration_test,
            WebData
        }
    };

    pub fn mock_create_workload_request() -> bson::Bson {
        bson::to_bson(&CreateWorkloadRequest {
            version: "1.0.0".to_string(),
            nix_pkg: "nix_pkg".to_string(),
            min_hosts: 1,
            system_specs: system_specs::SystemSpecDto {
                memory: 1024,
                disk: 1024,
                cores: 1,
            },
        }).unwrap()
    }


    #[actix_web::test]
    pub async fn should_successfully_create_workload_owned_by_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let user_id = bson::oid::ObjectId::new().to_string();
        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/workload")
            .set_json(mock_create_workload_request());

        let resp = perform_integration_test(
            create_workload::create_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                cache: None,
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_CREATE.to_string(),
                    ],
                }),
            }
        ).await.unwrap();

        let body = resp.body.unwrap();
        let owner_user_id = body.get("owner_user_id").unwrap().as_str().unwrap();
        assert_eq!(owner_user_id, user_id);
        assert_eq!(resp.status, StatusCode::CREATED);
    }

    #[actix_web::test]
    pub async fn should_fail_to_create_workload_if_not_authorized() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let user_id = bson::oid::ObjectId::new().to_string();
        let req = TestRequest::post()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/workload")
            .set_json(mock_create_workload_request());

        let resp = perform_integration_test(
            create_workload::create_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                cache: None,
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 1000000000,
                    permissions: vec![],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::FORBIDDEN);
    }
}