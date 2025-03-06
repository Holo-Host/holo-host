
#[cfg(test)]
mod tests {
    use actix_web::{http::StatusCode, test::TestRequest};
    use bson::oid::ObjectId;
    use mongodb::Database;

    use crate::{
        controllers::workloads::delete_workload,
        providers::{
            database::schemas::{
                shared::{
                    meta::new_meta,
                    system_specs::SystemSpec
                },
            workload
        },
        jwt::AccessTokenClaims,
        permissions::{WORKLOADS_DELETE, WORKLOADS_DELETE_ALL}
    },
    tests::utils::{
            get_app_config,
            get_db,
            perform_integration_test,
            WebData
        }
    };

    pub async fn create_workload(db: &Database) -> workload::Workload {
        let mut workload = workload::Workload{
            oid: None,
            meta: new_meta(),
            owner_user_id: ObjectId::new(),
            version: "1.0.0".to_string(),
            nix_pkg: "nixpkgs".to_string(),
            min_hosts: 1,
            system_specs: SystemSpec {
                memory: 1024,
                disk: 1024,
                cores: 1
            }
        };
        let workload_doc = bson::to_bson(&workload).unwrap();

        let result =db.collection(
            workload::WORKLOAD_COLLECTION_NAME
        ).insert_one(workload_doc, None).await.unwrap();

        let workload_id = result.inserted_id.as_object_id().unwrap();
        workload.oid = Some(workload_id);

        workload
    }
    
    #[actix_web::test]
    pub async fn should_successfully_delete_workload_owned_by_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload = create_workload(&db).await;
        let workload_id = workload.oid.map(|id| id.to_hex());
        let user_id = workload.owner_user_id;

        let req = TestRequest::delete()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", workload_id.unwrap()).as_str());

        let resp = perform_integration_test(
            delete_workload::delete_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 0,
                    permissions: vec![
                        WORKLOADS_DELETE.to_string()
                    ]
                })
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::OK);
    }

    #[actix_web::test]
    pub async fn should_successfully_delete_workload_owned_by_other_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload = create_workload(&db).await;
        let workload_id = workload.oid.map(|id| id.to_hex());

        let req = TestRequest::delete()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", workload_id.unwrap()).as_str());

        let resp = perform_integration_test(
            delete_workload::delete_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_hex(),
                    exp: 0,
                    permissions: vec![
                        WORKLOADS_DELETE_ALL.to_string()
                    ]
                })
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::OK);
    }

    #[actix_web::test]
    pub async fn should_fail_to_delete_workload_if_not_owned_by_user() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let workload = create_workload(&db).await;
        let workload_id = bson::oid::ObjectId::new().to_hex();
        let user_id = workload.owner_user_id;

        let req = TestRequest::delete()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", workload_id).as_str());

        let resp = perform_integration_test(
            delete_workload::delete_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: user_id.to_string(),
                    exp: 0,
                    permissions: vec![
                        WORKLOADS_DELETE.to_string()
                    ]
                })
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    pub async fn should_fail_to_delete_workload_if_not_authorized() {
        let app_config = get_app_config();
        let db = get_db(&app_config).await;

        let req = TestRequest::delete()
            .insert_header(("Content-Type", "application/json"))
            .uri(format!("/v1/workload/{}", bson::oid::ObjectId::new().to_hex()).as_str());

        let resp = perform_integration_test(
            delete_workload::delete_workload,
            req,
            WebData {
                config: Some(app_config),
                db: Some(db),
                auth: Some(AccessTokenClaims {
                    sub: bson::oid::ObjectId::new().to_hex(),
                    exp: 0,
                    permissions: vec![]
                })
            }
        ).await.unwrap();

        assert_eq!(resp.status, StatusCode::FORBIDDEN);
    }
}
