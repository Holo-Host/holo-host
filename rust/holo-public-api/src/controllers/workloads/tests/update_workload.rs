#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test::TestRequest};
    use mongodb::Database;

    use crate::{
        controllers::workloads::update_workload, providers::{
            database::schemas::{
                shared::{meta::new_meta, system_specs::SystemSpec},
                workload,
            },
            jwt::AccessTokenClaims, permissions::{
                WORKLOADS_UPDATE,
                WORKLOADS_UPDATE_ALL
            }
        },
        tests::utils::{
            get_app_config,
            get_db,
            perform_integration_test,
            WebData
        }
    };

    pub async fn create_test_workload(db: &Database, user_id: &str) -> workload::Workload {
        let mut workload = workload::Workload {
            oid: None,
            meta: new_meta(),
            owner_user_id: bson::oid::ObjectId::parse_str(&user_id).unwrap(),
            version: "1.0.0".to_string(),
            nix_pkg: "nix_pkg".to_string(),
            min_hosts: 1,
            system_specs: SystemSpec {
                memory: 1024,
                disk: 1024,
                cores: 1,
            },
        };

        let result = db.collection::<workload::Workload>(
            workload::WORKLOAD_COLLECTION_NAME
        ).insert_one(workload.clone(), None).await.unwrap();

        workload.oid = Some(result.inserted_id.as_object_id().unwrap());

        workload
    }

    #[actix_web::test]
    pub async fn should_successfully_update_workload_owned_by_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let user_id = bson::oid::ObjectId::new().to_string();
        let workload = create_test_workload(&db, &user_id).await;
        let workload_id = workload.oid.unwrap().to_string();

        let update_workload_request = update_workload::UpdateWorkloadRequest {
            version: Some("1.0.1".to_string()),
            nix_pkg: Some("nix_pkg_2".to_string()),
            min_hosts: Some(2),
            system_specs: None,
            owner_user_id: None,
        };

        let req = TestRequest::patch()
            .insert_header(("Content-Type", "application/json"))
            .uri(&format!("/v1/workload/{}", workload_id))
            .set_json(update_workload_request);

        let resp = perform_integration_test(
            update_workload::update_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_UPDATE.to_string(),
                    ],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::OK);
        let body = resp.body.unwrap();
        assert_eq!(body.get("version").unwrap().as_str().unwrap(), "1.0.1");
        assert_eq!(body.get("nix_pkg").unwrap().as_str().unwrap(), "nix_pkg_2");
        assert_eq!(body.get("min_hosts").unwrap().as_i32().unwrap(), 2);
    }

    #[actix_web::test]
    pub async fn should_successfully_update_workload_not_owned_by_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload = create_test_workload(
            &db, &bson::oid::ObjectId::new().to_string()
        ).await;
        let workload_id = workload.oid.unwrap().to_string();

        let update_workload_request = update_workload::UpdateWorkloadRequest {
            version: Some("1.0.1".to_string()),
            nix_pkg: Some("nix_pkg_2".to_string()),
            min_hosts: Some(2),
            system_specs: None,
            owner_user_id: None,
        };

        let req = TestRequest::patch()
            .insert_header(("Content-Type", "application/json"))
            .uri(&format!("/v1/workload/{}", workload_id))
            .set_json(update_workload_request);

        let resp = perform_integration_test(
            update_workload::update_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_UPDATE_ALL.to_string(),
                    ],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::OK);
        let body = resp.body.unwrap();
        assert_eq!(body.get("version").unwrap().as_str().unwrap(), "1.0.1");
        assert_eq!(body.get("nix_pkg").unwrap().as_str().unwrap(), "nix_pkg_2");
        assert_eq!(body.get("min_hosts").unwrap().as_i32().unwrap(), 2);
    }

    #[actix_web::test]
    pub async fn should_fail_to_update_workload_if_not_owned_by_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let user_id = bson::oid::ObjectId::new().to_string();
        let workload = create_test_workload(&db, &user_id).await;
        let workload_id = workload.oid.unwrap().to_string();

        let update_workload_request = update_workload::UpdateWorkloadRequest {
            version: Some("1.0.1".to_string()),
            nix_pkg: None,
            min_hosts: None,
            system_specs: None,
            owner_user_id: None,
        };

        let req = TestRequest::patch()
            .insert_header(("Content-Type", "application/json"))
            .uri(&format!("/v1/workload/{}", workload_id))
            .set_json(update_workload_request);

        let resp = perform_integration_test(
            update_workload::update_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_UPDATE.to_string(),
                    ],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    pub async fn should_fail_to_update_workload_if_not_authorized() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let user_id = bson::oid::ObjectId::new().to_string();
        let workload = create_test_workload(&db, &user_id).await;
        let workload_id = workload.oid.unwrap().to_string();

        let update_workload_request = update_workload::UpdateWorkloadRequest {
            version: Some("1.0.1".to_string()),
            nix_pkg: None,
            min_hosts: None,
            system_specs: None,
            owner_user_id: None,
        };

        let req = TestRequest::patch()
            .insert_header(("Content-Type", "application/json"))
            .uri(&format!("/v1/workload/{}", workload_id))
            .set_json(update_workload_request);

        let resp = perform_integration_test(
            update_workload::update_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 1000000000,
                    permissions: vec![],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    pub async fn should_fail_to_update_workload_if_not_found() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload_id = bson::oid::ObjectId::new().to_string();

        let req = TestRequest::patch()
            .insert_header(("Content-Type", "application/json"))
            .uri(&format!("/v1/workload/{}", workload_id))
            .set_json(bson::doc!{});

        let resp = perform_integration_test(
            update_workload::update_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_UPDATE.to_string(),
                    ],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }
}