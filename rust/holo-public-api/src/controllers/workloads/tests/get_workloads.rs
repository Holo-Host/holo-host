#[cfg(test)]
mod tests {
    use actix_web::http::StatusCode;
    use actix_web::test::TestRequest;

    use crate::providers::database::schemas::shared::meta::new_meta;
    use crate::providers::database::schemas::shared::system_specs;
    use crate::providers::database::schemas::workload::{Workload, WORKLOAD_COLLECTION_NAME};
    use crate::providers::jwt::AccessTokenClaims;
    use crate::providers::permissions::{WORKLOADS_READ, WORKLOADS_READ_ALL};
    use crate::tests::utils::{get_app_config, get_db, perform_integration_test, WebData};
    use crate::controllers::workloads::get_workloads::get_workloads;
    
    #[actix_web::test]
    async fn should_succeed_to_fetch_user_workloads() {
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/workloads?page=1&limit=10");

        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        // user_id
        let user_id = bson::oid::ObjectId::new();

        // insert a workload
        db.collection::<Workload>(
            WORKLOAD_COLLECTION_NAME
        ).insert_many(
            vec![
                Workload{
                    _id: None,
                    _meta: new_meta(),
                    owner_user_id: user_id,
                    version: "1.0.0".to_string(),
                    nix_pkg: "test".to_string(),
                    min_hosts: 1,
                    system_specs: system_specs::SystemSpec {
                        memory: 1024,
                        disk: 1024,
                        cores: 1,
                    },
                },
                Workload{
                    _id: None,
                    _meta: new_meta(),
                    owner_user_id: bson::oid::ObjectId::new(),
                    version: "1.0.0".to_string(),
                    nix_pkg: "test".to_string(),
                    min_hosts: 1,
                    system_specs: system_specs::SystemSpec {
                        memory: 1024,
                        disk: 1024,
                        cores: 1,
                    },
                }
            ], None).await.unwrap();
        
        let resp = perform_integration_test(
            get_workloads,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_READ.to_string(),
                    ],
                }),
            }
        ).await.unwrap();

        // check the response
        let body = resp.body.clone().unwrap();
        let items = body.get("items").unwrap().as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(resp.status, StatusCode::OK);
    }

    #[actix_web::test]
    async fn should_succeed_to_fetch_all_workloads() {
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/workloads?page=1&limit=10");

        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        db.collection::<Workload>(
            WORKLOAD_COLLECTION_NAME
        ).insert_many(
            vec![
                Workload{
                    _id: None,
                    _meta: new_meta(),
                    owner_user_id: bson::oid::ObjectId::new(),
                    version: "1.0.0".to_string(),
                    nix_pkg: "test".to_string(),
                    min_hosts: 1,
                    system_specs: system_specs::SystemSpec {
                        memory: 1024,
                        disk: 1024,
                        cores: 1,
                    },
                }
            ], None).await.unwrap();

        let resp = perform_integration_test(
            get_workloads,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_READ_ALL.to_string(),
                    ],
                }),
            }
        ).await.unwrap();


        let body = resp.body.clone().unwrap();
        let items = body.get("items").unwrap().as_array().unwrap();
        assert!(items.len() >= 1);
        assert_eq!(resp.status, StatusCode::OK);
    }

    #[actix_web::test]
    async fn should_fail_without_permissions() {
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/workloads?page=1&limit=10");

        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let resp = perform_integration_test(
            get_workloads,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn should_fail_with_invalid_limit() {
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri("/v1/workloads?page=1&limit=101");

        let app_config = get_app_config();
        let db = get_db(&app_config).await;
        
        let resp = perform_integration_test(
            get_workloads,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::BAD_REQUEST);
    }
}