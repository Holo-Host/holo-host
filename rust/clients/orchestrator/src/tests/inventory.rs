#[cfg(tests_inventory)]
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bson::{doc, oid::ObjectId};
    use db_utils::mongodb::MongoDbAPI;
    use db_utils::{
        mongodb::MongoCollection,
        schemas::{self, Host, PubKey, Workload},
    };
    use futures::StreamExt;
    use hpos_hal::inventory::HoloInventory;
    use inventory::{
        types::InventoryUpdateStatus, INVENTORY_SRV_DESC, INVENTORY_SRV_NAME, INVENTORY_SRV_SUBJ,
        INVENTORY_SRV_VERSION, INVENTORY_UPDATE_SUBJECT,
    };
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

    const TEST_ADMIN_CLIENT_NAME: &str = "Test Admin Client";
    const TEST_ADMIN_INBOX_PREFIX: &str = "_TEST_ADMIN_INBOX";

    async fn setup_test_environment() -> Result<(TestNatsServer, MongoDBClient)> {
        let nats_server = TestNatsServer::new()
            .await
            .expect("Failed to start test nats server.");
        println!("Nats Server is running at: {:?}", nats_server.get_url());
        let mongod = MongodRunner::run().expect("Failed to start test mongodb server.");
        let mongo_client = mongod
            .client()
            .expect("Failed to connect client to test mongodb server.");
        Ok((nats_server, mongo_client))
    }

    async fn init_inventory_service(
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

        // Register inventory service
        let inventory_stream_service = nats_utils::types::JsServiceBuilder {
            name: INVENTORY_SRV_NAME.to_string(),
            description: INVENTORY_SRV_DESC.to_string(),
            version: INVENTORY_SRV_VERSION.to_string(),
            service_subject: INVENTORY_SRV_SUBJ.to_string(),
        };

        client
            .add_js_service(inventory_stream_service)
            .await
            .expect("Failed to add inventory service to Jetstream Client");

        let service = client.get_js_service(INVENTORY_SRV_NAME.to_string()).await;
        assert!(service.is_some());
        service.unwrap().to_owned()
    }

    async fn setup_test_host(mongo_client: &MongoDBClient, host_pubkey: &str) -> Result<ObjectId> {
        let host_collection = MongoCollection::<Host>::new(
            mongo_client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await?;

        let mut host = Host::default();
        host.device_id = PubKey::from(host_pubkey);
        let host_id = host_collection.insert_one_into(host).await?;
        Ok(host_id)
    }

    #[tokio::test]
    #[serial]
    async fn test_inventory_service_initialization() {
        let (nats_server, _) = setup_test_environment().await.unwrap();

        let service = init_inventory_service(&nats_server).await;

        let stream_info = service.get_service_info();
        assert_eq!(stream_info.name, INVENTORY_SRV_NAME);
        assert_eq!(stream_info.version, INVENTORY_SRV_VERSION);
        assert_eq!(stream_info.service_subject, INVENTORY_SRV_SUBJ);

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_inventory_consumer_registration() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let orchestrator_client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let _ = crate::inventory::run(orchestrator_client, mongo_client)
            .await
            .expect("Failed to run inventory service");

        // Verify the consumer subject is properly registered
        let service = init_inventory_service(&nats_server).await;

        let consumer_info = service
            .get_consumer_stream_info("update_host_inventory")
            .await
            .unwrap();

        assert!(
            consumer_info.is_some(),
            "Consumer update_host_inventory not found"
        );
        let consumer_info = consumer_info.unwrap();
        assert_eq!(
            consumer_info.config.filter_subject,
            format!("INVENTORY.{}", INVENTORY_UPDATE_SUBJECT)
        );

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_inventory_update_handling() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let orchestrator_client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let _ = crate::inventory::run(orchestrator_client.clone(), mongo_client.clone())
            .await
            .expect("Failed to run inventory service");

        // Create mock inventory data and host
        let mock_inventory = HoloInventory::default();
        let host_pubkey = "test_host_pubkey";
        let _host_id = setup_test_host(&mongo_client, host_pubkey).await.unwrap();

        // Publish inventory update message
        let publish_info = PublishInfo {
            subject: format!("INVENTORY.{}.{}", host_pubkey, INVENTORY_UPDATE_SUBJECT),
            msg_id: "update_inventory_id".to_string(),
            data: serde_json::to_vec(&mock_inventory).unwrap(),
            headers: None,
        };

        orchestrator_client
            .publish(publish_info)
            .await
            .expect("Failed to publish inventory update message");

        // Wait for message processing
        sleep(Duration::from_secs(1)).await;

        // Verify the inventory was stored in MongoDB
        let host_collection = MongoCollection::<Host>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await
        .unwrap();

        let stored_host = host_collection
            .inner
            .find_one(doc! { "device_id": host_pubkey })
            .await
            .expect("Error locating host");

        assert!(stored_host.is_some());
        let stored_host = stored_host.unwrap();
        assert!(stored_host.inventory.drives.len() == mock_inventory.drives.len());

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_inventory_update_with_workload_reallocation() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let orchestrator_client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let _ = crate::inventory::run(orchestrator_client.clone(), mongo_client.clone())
            .await
            .expect("Failed to run inventory service");

        // Setup test host and workload
        let host_pubkey = "test_host_pubkey";
        let host_id = setup_test_host(&mongo_client, host_pubkey).await.unwrap();

        // Create a workload that exceeds host capacity
        let workload_collection = MongoCollection::<Workload>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::WORKLOAD_COLLECTION_NAME,
        )
        .await
        .unwrap();

        let mut workload = Workload::default();
        workload.assigned_hosts.push(host_id);
        workload.system_specs.capacity.cores = 999; // Unrealistic number to ensure it exceeds host capacity
        let workload_id = workload_collection.insert_one_into(workload).await.unwrap();

        // Update host to reference workload
        let host_collection = MongoCollection::<Host>::new(
            &mongo_client,
            schemas::DATABASE_NAME,
            schemas::HOST_COLLECTION_NAME,
        )
        .await
        .unwrap();

        host_collection
            .inner
            .update_one(
                doc! { "_id": host_id },
                doc! { "$push": { "assigned_workloads": workload_id } },
            )
            .await
            .unwrap();

        // Create and publish inventory update
        let mock_inventory = HoloInventory::default(); // Has minimal resources
        let publish_info = PublishInfo {
            subject: format!("INVENTORY.{}.{}", host_pubkey, INVENTORY_UPDATE_SUBJECT),
            msg_id: "update_inventory_id".to_string(),
            data: serde_json::to_vec(&mock_inventory).unwrap(),
            headers: None,
        };

        orchestrator_client
            .publish(publish_info)
            .await
            .expect("Failed to publish inventory update message");

        // Wait for message processing
        sleep(Duration::from_secs(1)).await;

        // Verify workload was removed from host
        let updated_host = host_collection
            .inner
            .find_one(doc! { "_id": host_id })
            .await
            .unwrap()
            .unwrap();

        assert!(!updated_host.assigned_workloads.contains(&workload_id));

        // Verify host was removed from workload
        let updated_workload = workload_collection
            .inner
            .find_one(doc! { "_id": workload_id })
            .await
            .unwrap()
            .unwrap();

        assert!(!updated_workload.assigned_hosts.contains(&host_id));

        let _ = nats_server.shutdown().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_inventory_update_response() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let orchestrator_client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let _ = crate::inventory::run(orchestrator_client.clone(), mongo_client.clone())
            .await
            .expect("Failed to run inventory service");

        // Setup test host
        let host_pubkey = "test_host_pubkey";
        let _host_id = setup_test_host(&mongo_client, host_pubkey).await.unwrap();

        // Create mock inventory data
        let mock_inventory = HoloInventory::default();

        // Set up subscriber for response
        let TestClientResponse { client, js: _ } =
            nats_server.connect(&nats_server.port).await.unwrap();
        let inventory_subject = format!("INVENTORY.response.{}", host_pubkey);
        let s = client.subscribe(inventory_subject).await;
        assert!(s.is_ok());
        let mut subscriber = s.expect("Failed to create subscriber.");
        subscriber.unsubscribe_after(1).await.unwrap();

        tokio::spawn(async move {
            let msg_option_result = subscriber.next().await;
            assert!(msg_option_result.is_some());

            let msg = msg_option_result.unwrap();
            let response = serde_json::from_slice::<InventoryUpdateStatus>(&msg.payload)
                .expect("Failed to parse inventory response");

            assert!(matches!(response, InventoryUpdateStatus::Ok));

            let _ = nats_server.shutdown().await;
        });

        // Publish inventory update
        let publish_info = PublishInfo {
            subject: format!("INVENTORY.{}.{}", host_pubkey, INVENTORY_UPDATE_SUBJECT),
            msg_id: "update_inventory_id".to_string(),
            data: serde_json::to_vec(&mock_inventory).unwrap(),
            headers: None,
        };

        orchestrator_client
            .publish(publish_info)
            .await
            .expect("Failed to publish inventory update message");

        // Wait for message processing
        sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_inventory_service_shutdown() {
        let (nats_server, mongo_client) = setup_test_environment().await.unwrap();

        let client = crate::admin_client::run(&None, nats_server.get_url())
            .await
            .unwrap();

        let _ = crate::inventory::run(client.clone(), mongo_client)
            .await
            .expect("Failed to run inventory service");

        // Test graceful shutdown
        let result = client.close().await;
        assert!(result.is_ok());

        let _ = nats_server.shutdown().await;
    }
}
