
#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test::TestRequest};
    use mongodb::Database;

    use crate::{
        controllers::workloads::get_single_workload,
        providers::{
            database::schemas::{
                workload,
                shared::{
                    meta::new_meta,
                    system_specs::SystemSpec
                },
            },
            jwt::AccessTokenClaims,
            permissions::WORKLOADS_READ
        },
        tests::utils::{
            get_app_config,
            get_db,
            perform_integration_test,
            WebData
        }
    };

    async fn create_workload(db: &Database) -> workload::Workload {
        let user_id = bson::oid::ObjectId::new();

        let mut workload = workload::Workload {
            oid: None,
            meta: new_meta(),
            owner_user_id: user_id,
            version: "1.0.0".to_string(),
            nix_pkg: "test".to_string(),
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

        let workload_id = result.inserted_id.as_object_id().unwrap();
        workload.oid = Some(workload_id);

        workload
    }

    #[actix_web::test]
    async fn should_succeed_to_fetch_single_workload() {

        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload = create_workload(&db).await;
        let workload_id = workload.oid;
        let user_id = workload.owner_user_id;
        
        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", workload_id.unwrap().to_hex()).as_str());

        let resp = perform_integration_test(
            get_single_workload::get_single_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
                cache: None,
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_READ.to_string()
                    ],
                }),
            }
        ).await.unwrap();

        let body = resp.body.unwrap();
        assert_eq!(resp.status, StatusCode::OK);
        assert_eq!(
            body.get("id").unwrap().as_str().unwrap(),
            workload_id.unwrap().to_hex()
        );
    }

    #[actix_web::test]
    async fn should_fail_to_fetch_single_workload_if_not_authorized() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload = create_workload(&db).await;
        let workload_id = workload.oid;
        let user_id = workload.owner_user_id;

        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", workload_id.unwrap().to_hex()).as_str());

        let resp = perform_integration_test(
            get_single_workload::get_single_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
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

    #[actix_web::test]
    async fn should_fail_if_workload_does_not_exist() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let req = TestRequest::get()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", bson::oid::ObjectId::new().to_hex()).as_str());

        let resp = perform_integration_test(
            get_single_workload::get_single_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db.clone()),
                cache: None,
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_string(),
                    exp: 1000000000,
                    permissions: vec![
                        WORKLOADS_READ.to_string()
                    ],
                }),
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }
}
