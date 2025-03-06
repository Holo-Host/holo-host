#[cfg(not(target_arch = "aarch64"))]
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bson::doc;
    use bson::oid::ObjectId;
    use db_utils::{
        mongodb::MongoCollection,
        schemas::{self, Workload, WorkloadState, WorkloadStatus},
    };
    use futures::StreamExt;
    use mock_utils::{
        mongodb_runner::MongodRunner,
        test_nats_server::{TestClientResponse, TestNatsServer},
    };
    use mongodb::Client as MongoDBClient;
    use nats_utils::{
        jetstream_client::{get_event_listeners, with_event_listeners, JsClient},
        types::{JsClientBuilder, PublishInfo},
    };
    use serial_test::serial;
    use std::time::Duration;
    use tokio::time::sleep;
    use workload::types::WorkloadResult;
    use workload::{
        types::WorkloadServiceSubjects, WORKLOAD_SRV_DESC, WORKLOAD_SRV_NAME, WORKLOAD_SRV_SUBJ,
        WORKLOAD_SRV_VERSION,
    };

    const TEST_ADMIN_CLIENT_NAME: &str = "Test Admin Client";
    const TEST_ADMIN_INBOX_PREFIX: &str = "_TEST_ADMIN_INBOX";

    async fn setup_test_environment() -> Result<(TestNatsServer, MongoDBClient)> {
        let nats_server = TestNatsServer::new().await.unwrap();
        println!("Nats Server is running at: {:?}", nats_server.get_url());
        let mongod = MongodRunner::run().unwrap();
        let mongo_client = mongod.client().unwrap();
        Ok((nats_server, mongo_client))
    }

    async fn init_workload_service(
        nats_server: &TestNatsServer,
    ) -> nats_utils::jetstream_service::JsStreamService {
        let mut client = JsClient::new(JsClientBuilder {
            nats_url: nats_server.get_url(),
            name: TEST_ADMIN_CLIENT_NAME.to_string(),
            inbox_prefix: TEST_ADMIN_INBOX_PREFIX.to_string(),
            credentials: None, // No credentials needed for test server
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
            listeners: vec![with_event_listeners(get_event_listeners())],
        })
        .await
        .expect("Failed to spin up Jetstream Client");

        // Register workload service
        let workload_stream_service = nats_utils::types::JsServiceBuilder {
            name: WORKLOAD_SRV_NAME.to_string(),
            description: WORKLOAD_SRV_DESC.to_string(),
            version: WORKLOAD_SRV_VERSION.to_string(),
            service_subject: WORKLOAD_SRV_SUBJ.to_string(),
        };

        client
            .add_js_service(workload_stream_service)
            .await
            .expect("Failed to add workload service to Jetstream Client");

        let service = client.get_js_service(WORKLOAD_SRV_NAME.to_string()).await;
        assert!(service.is_some());
        service.unwrap().to_owned()
    }

    #[tokio::test]
    #[serial]
    async fn test_workload_service_initialization() {
        let (nats_server, _) = setup_test_environment().await.unwrap();

        let service = init_workload_service(&nats_server).await;

        let stream_info = service.get_service_info();
        assert_eq!(stream_info.name, WORKLOAD_SRV_NAME);
        assert_eq!(stream_info.version, WORKLOAD_SRV_VERSION);
        assert_eq!(stream_info.service_subject, WORKLOAD_SRV_SUBJ);

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_workload_consumer_registration() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let orchestrator_client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let client = crate::workloads::run(orchestrator_client, mongo_client)
            .await
            .expect("Failed to run workload service");

        // Verify each consumer subject is properly registered
        let subjects = [
            ("add_workload", WorkloadServiceSubjects::Add),
            ("update_workload", WorkloadServiceSubjects::Update),
            ("delete_workload", WorkloadServiceSubjects::Delete),
            ("handle_db_insertion", WorkloadServiceSubjects::Insert),
            ("handle_db_modification", WorkloadServiceSubjects::Modify),
            (
                "handle_status_update",
                WorkloadServiceSubjects::HandleStatusUpdate,
            ),
        ];

        let service = client
            .get_js_service(WORKLOAD_SRV_NAME.to_string())
            .await
            .expect("Failed to locate Workload Service");

        for (consumer_name, subject) in subjects.iter() {
            let consumer_info = service
                .get_consumer_stream_info(consumer_name)
                .await
                .unwrap();
            assert!(
                consumer_info.is_some(),
                "Consumer {} not found",
                consumer_name
            );

            let consumer_info = consumer_info.unwrap();
            assert_eq!(
                consumer_info.config.filter_subject,
                format!("WORKLOAD.{}", subject.as_ref())
            );
        }

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_add_workload_request() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let orchestrator_client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let client = crate::workloads::run(orchestrator_client, mongo_client.clone())
            .await
            .expect("Failed to run workload service");

        let mut mock_workload = Workload::default();
        let mock_developer_id = ObjectId::new();
        mock_workload.assigned_developer = mock_developer_id;

        // Publish the add workload message
        let publish_info = PublishInfo {
            subject: format!("WORKLOAD.{}", WorkloadServiceSubjects::Add.as_ref()),
            msg_id: "add_workload_id".to_string(),
            data: serde_json::to_vec(&mock_workload).unwrap().into(),
            headers: None,
        };

        client
            .publish(publish_info)
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");
        println!("Published insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;

        // Fetch the workload from the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();

        let workload = workload_collection
            .inner
            .find_one(doc! { "assigned_developer": mock_workload.assigned_developer })
            .await
            .expect("Error locating workload with assigned devloper.");
        println!("workload ? : {:?}", workload);
        assert!(workload.is_some());

        let workload = workload.expect(&format!(
            "Failed to return workload with assigned developer {:?}",
            mock_workload.assigned_developer,
        ));
        assert!(matches!(workload.status.desired, WorkloadState::Running));
        assert!(matches!(workload.status.actual, WorkloadState::Reported));
    }

    #[tokio::test]
    #[serial]
    async fn test_update_workload_request() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        // Generate a mock workload to be inserted into the database
        let mut mock_workload = Workload::default();
        let mock_workload_id = ObjectId::new();
        mock_workload._id = Some(mock_workload_id);

        // Insert the workload into the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        workload_collection
            .inner
            .insert_one(mock_workload)
            .await
            .unwrap();

        // Publish the add workload message
        let publish_info = PublishInfo {
            subject: format!("WORKLOAD.{}", WorkloadServiceSubjects::Update.as_ref()),
            msg_id: "update_workload_id".to_string(),
            data: serde_json::to_vec(&mock_workload_id).unwrap().into(),
            headers: None,
        };
        client
            .publish(publish_info)
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;

        // Fetch the workload from the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        let workload = workload_collection
            .inner
            .find_one(doc! { "_id": mock_workload_id })
            .await
            .unwrap();
        assert!(workload.is_some());

        let workload = workload.unwrap();
        assert!(matches!(workload.status.desired, WorkloadState::Updated));
        assert!(matches!(workload.status.actual, WorkloadState::Updating));
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_workload_request() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let mut mock_workload = Workload::default();
        let mock_developer_id = ObjectId::new();
        mock_workload.assigned_developer = mock_developer_id;

        // Publish the add workload message
        let publish_info = PublishInfo {
            subject: format!("WORKLOAD.{}", WorkloadServiceSubjects::Delete.as_ref()),
            msg_id: "delete_workload_id".to_string(),
            data: serde_json::to_vec(&mock_workload).unwrap().into(),
            headers: None,
        };
        client
            .publish(publish_info)
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;

        // Fetch the workload from the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        let workload = workload_collection
            .inner
            .find_one(doc! { "assigned_developer": mock_workload.assigned_developer })
            .await
            .unwrap();
        assert!(workload.is_some());

        let workload = workload.unwrap();
        assert!(matches!(workload.status.desired, WorkloadState::Removed));
        assert!(matches!(workload.status.actual, WorkloadState::Deleted));
    }

    #[tokio::test]
    #[serial]
    async fn test_handling_workload_insertion() {
        let (nats_server, _) = setup_test_environment().await.unwrap();

        let _client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        // Generate a mock workload to be inserted into the database
        let mut mock_inserted_workload = Workload::default();
        let mock_inserted_workload_id = ObjectId::new();
        mock_inserted_workload._id = Some(mock_inserted_workload_id);

        // Spawn the subcription to the consumer's response subject
        let TestClientResponse { client, js: _ } =
            nats_server.connect(&nats_server.port).await.unwrap();
        let s = client
            .subscribe(WorkloadServiceSubjects::Install.as_ref().to_string())
            .await;
        assert!(s.is_ok());
        let mut subscriber = s.expect("Failed to create subscriber.");
        subscriber.unsubscribe_after(1).await.unwrap();

        tokio::spawn(async move {
            let msg_option_result = subscriber.next().await;
            assert!(msg_option_result.is_some());

            let msg = msg_option_result.unwrap();
            let test_str_payload = std::str::from_utf8(&msg.payload).expect("Invalid UTF-8");
            let test_insert_payload = serde_json::from_str::<WorkloadResult>(test_str_payload)
                .expect("Failed to convert str to WorkloadResult");
            assert!(test_insert_payload.workload.is_some());
            assert_eq!(
                test_insert_payload.status.desired.as_ref().to_string(),
                WorkloadState::Running.as_ref().to_string()
            );
            assert_eq!(
                test_insert_payload.status.actual.as_ref().to_string(),
                WorkloadState::Assigned.as_ref().to_string()
            );
            assert_eq!(
                test_insert_payload.status.id,
                Some(mock_inserted_workload_id)
            );

            let _ = nats_server.shutdown().await;
        });

        // Publish the insert workload message
        client
            .publish(
                WorkloadServiceSubjects::Insert.as_ref().to_string(),
                serde_json::to_vec(&mock_inserted_workload).unwrap().into(),
            )
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_handling_workload_modification() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let _client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        // Generate a new workload to later mod
        let mut mock_modified_workload = Workload::default();
        let mock_modified_workload_id = ObjectId::new();
        mock_modified_workload._id = Some(mock_modified_workload_id);
        mock_modified_workload.status.desired = WorkloadState::Updated;
        mock_modified_workload.status.actual = WorkloadState::Updating;

        // Insert the workload into the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        workload_collection
            .inner
            .insert_one(mock_modified_workload.clone())
            .await
            .unwrap();

        // Spawn the subcription to the consumer's response subject
        let TestClientResponse { client, js: _ } =
            nats_server.connect(&nats_server.port).await.unwrap();
        let s = client
            .subscribe(
                WorkloadServiceSubjects::UpdateInstalled
                    .as_ref()
                    .to_string(),
            )
            .await;
        assert!(s.is_ok());
        let mut subscriber = s.expect("Failed to create subscriber.");
        subscriber.unsubscribe_after(1).await.unwrap();

        tokio::spawn(async move {
            let msg_option_result = subscriber.next().await;
            assert!(msg_option_result.is_some());

            let msg = msg_option_result.unwrap();
            let test_str_payload = std::str::from_utf8(&msg.payload).expect("Invalid UTF-8");
            let test_insert_payload = serde_json::from_str::<WorkloadResult>(test_str_payload)
                .expect("Failed to convert str to WorkloadResult");
            assert!(test_insert_payload.workload.is_some());
            assert_eq!(
                test_insert_payload.status.desired.as_ref().to_string(),
                WorkloadState::Running.as_ref().to_string()
            );
            assert_eq!(
                test_insert_payload.status.actual.as_ref().to_string(),
                WorkloadState::Updated.as_ref().to_string()
            );
            assert_eq!(
                test_insert_payload.status.id,
                Some(mock_modified_workload_id)
            );

            let _ = nats_server.shutdown().await;
        });

        // Publish the modify workload message
        client
            .publish(
                WorkloadServiceSubjects::Modify.as_ref().to_string(),
                serde_json::to_vec(&mock_modified_workload).unwrap().into(),
            )
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_handling_workload_delete_modification() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let _client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        // Generate a new workload to later mod
        let mut mock_modified_workload = Workload::default();
        let mock_modified_workload_id = ObjectId::new();
        mock_modified_workload._id = Some(mock_modified_workload_id);
        mock_modified_workload.status.desired = WorkloadState::Removed;
        mock_modified_workload.status.actual = WorkloadState::Deleted;

        // Insert the workload into the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        workload_collection
            .inner
            .insert_one(mock_modified_workload.clone())
            .await
            .unwrap();

        // Spawn the subcription to the consumer's response subject
        let TestClientResponse { client, js: _ } =
            nats_server.connect(&nats_server.port).await.unwrap();
        let s = client
            .subscribe(
                WorkloadServiceSubjects::UpdateInstalled
                    .as_ref()
                    .to_string(),
            )
            .await;
        assert!(s.is_ok());
        let mut subscriber = s.expect("Failed to create subscriber.");
        subscriber.unsubscribe_after(1).await.unwrap();

        tokio::spawn(async move {
            let msg_option_result = subscriber.next().await;
            assert!(msg_option_result.is_some());

            let msg = msg_option_result.unwrap();
            let test_str_payload = std::str::from_utf8(&msg.payload).expect("Invalid UTF-8");
            let test_insert_payload = serde_json::from_str::<WorkloadResult>(test_str_payload)
                .expect("Failed to convert str to WorkloadResult");
            assert!(test_insert_payload.workload.is_some());
            assert_eq!(
                test_insert_payload.status.desired.as_ref().to_string(),
                WorkloadState::Uninstalled.as_ref().to_string()
            );
            assert_eq!(
                test_insert_payload.status.actual.as_ref().to_string(),
                WorkloadState::Removed.as_ref().to_string()
            );
            assert_eq!(
                test_insert_payload.status.id,
                Some(mock_modified_workload_id)
            );

            let _ = nats_server.shutdown().await;
        });

        // Publish the insert workload message
        client
            .publish(
                WorkloadServiceSubjects::Modify.as_ref().to_string(),
                serde_json::to_vec(&mock_modified_workload).unwrap().into(),
            )
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_handling_status_update() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        // Generate a mock workload to be inserted into the database
        let mut mock_workload = Workload::default();
        let mock_workload_id = ObjectId::new();
        mock_workload._id = Some(mock_workload_id);
        mock_workload.status.desired = WorkloadState::Running;
        mock_workload.status.actual = WorkloadState::Reported;

        // Insert the workload into the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        workload_collection
            .inner
            .insert_one(mock_workload)
            .await
            .unwrap();

        // Publish the add workload message
        let mock_workload_status = WorkloadStatus {
            id: Some(mock_workload_id),
            desired: WorkloadState::Running,
            actual: WorkloadState::Running,
        };
        let publish_info = PublishInfo {
            subject: format!(
                "WORKLOAD.{}",
                WorkloadServiceSubjects::HandleStatusUpdate.as_ref()
            ),
            msg_id: "update_workload_status_id".to_string(),
            data: serde_json::to_vec(&mock_workload_status).unwrap().into(),
            headers: None,
        };
        client
            .publish(publish_info)
            .await
            .expect("Failed to publish insert workload message on Jetstream Service");

        // Wait a sec for message processing
        sleep(Duration::from_secs(1)).await;

        // Fetch the workload from the database
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();
        let workload = workload_collection
            .inner
            .find_one(doc! { "_id": mock_workload_id })
            .await
            .unwrap();
        assert!(workload.is_some());

        let workload = workload.unwrap();
        assert!(matches!(workload.status.desired, WorkloadState::Running));
        assert!(matches!(workload.status.actual, WorkloadState::Running));

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_workload_service_shutdown() {
        let (nats_server, _) = setup_test_environment().await.unwrap();
        // let admin_creds_path = PathBuf::from_str(&jetstream_client::get_nats_creds_by_nsc(
        //     "HOLO", "ADMIN", "admin",
        // ))
        // .map(Credentials::Path)
        // .map_err(|e| anyhow!("Failed to locate admin credential path. Err={:?}", e)).unwrap();

        let client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        // Test graceful shutdown
        let result = client.close().await;
        assert!(result.is_ok());

        let _ = nats_server.shutdown().await;
    }
}
