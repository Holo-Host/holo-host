#[cfg(test)]
mod tests {
    use crate::InventoryServiceApi;
    use anyhow::Result;
    use bson::doc;
    use bson::oid::ObjectId;
    use db_utils::mongodb::api::MongoDbAPI;
    use db_utils::schemas::{host::Host, workload::Capacity};
    use mock_utils::{
        host::create_mock_inventory, mongodb_runner::MongodRunner, nats_message::NatsMessage,
        workload::create_test_workload,
    };
    use serial_test::serial;
    use std::sync::Arc;

    #[ctor::ctor]
    fn init() {
        dotenv::dotenv().ok();
        env_logger::init();
    }

    #[tokio::test]
    #[serial]

    async fn test_handle_authenticated_inventory_update() -> Result<()> {
        let mongod = MongodRunner::run().await?;
        let db_client = mongod.client();
        std::env::set_var("HOLO_DATABASE_NAME", mongod.db_name());
        let api = InventoryServiceApi::new(db_client).await?;

        // Create a host id to reference in the workload collection
        let host_id = ObjectId::new();

        // Create workload with specific requirements
        let workload = create_test_workload(
            None,
            Some(vec![host_id]),
            Some(1),
            Some(Capacity {
                drive: 500,
                cores: 16,
            }),
            Some(100),
            Some(0.9),
        );

        let workload_id = api.workload_collection.insert_one_into(workload).await?;

        // Create initial host with reference to workload (id) created above
        let initial_inventory = create_mock_inventory(Some(1000), Some(3), Some(20));

        let host = Host {
            _id: Some(host_id),
            device_id: format!("machine_id_{host_id}"),
            inventory: initial_inventory.clone(),
            assigned_workloads: vec![workload_id],
            ..Default::default()
        };
        api.host_collection.insert_one_into(host.clone()).await?;

        // Test that inventory update still meets workload requirements
        let inventory_update = create_mock_inventory(Some(2000), Some(3), Some(20));
        let msg_payload = serde_json::to_vec(&inventory_update)?;
        let msg = Arc::new(
            NatsMessage::new(format!("INVENTORY.{}.update", host.device_id), msg_payload)
                .into_message(),
        );

        let result = api.handle_host_inventory_update(msg).await?;

        assert!(matches!(
            result.status,
            crate::types::InventoryUpdateStatus::Ok
        ));

        // Verify host inventory was updated in db
        let updated_host = api
            .host_collection
            .get_one_from(doc! { "_id": host_id })
            .await?
            .expect("Failed to fetch updated host");

        assert_eq!(updated_host.assigned_workloads.len(), 1);
        assert!(updated_host.assigned_workloads.contains(&workload_id));

        // Clean up database
        mongod.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    #[serial]

    async fn test_handle_inventory_update_with_insufficient_resources() -> Result<()> {
        let mongod = MongodRunner::run().await?;
        let db_client = mongod.client();
        std::env::set_var("HOLO_DATABASE_NAME", mongod.db_name());
        let api = InventoryServiceApi::new(db_client).await?;

        // Create a host id to reference in the workload collection
        let host_id = ObjectId::new();

        // Create workload with specific requirements
        let workload = create_test_workload(
            None,
            Some(vec![host_id]),
            Some(1),
            Some(Capacity {
                drive: 500,
                cores: 16,
            }),
            Some(100),
            Some(0.9),
        );
        let workload_id = api.workload_collection.insert_one_into(workload).await?;

        // Create initial host with reference to workload (id) created above
        let initial_inventory = create_mock_inventory(Some(1000), Some(3), Some(20));
        let host = Host {
            _id: Some(host_id),
            device_id: format!("machine_id_{host_id}"),
            assigned_workloads: vec![workload_id],
            inventory: initial_inventory.clone(),
            ..Default::default()
        };
        api.host_collection.insert_one_into(host.clone()).await?;

        // Test inventory update with insufficient resources
        let insufficient_inventory = create_mock_inventory(Some(100), Some(1), Some(4));
        let msg_payload = serde_json::to_vec(&insufficient_inventory)?;
        let msg = Arc::new(
            NatsMessage::new(format!("INVENTORY.{}.update", host.device_id), msg_payload)
                .into_message(),
        );

        let result = api.handle_host_inventory_update(msg).await?;
        assert!(matches!(
            result.status,
            crate::types::InventoryUpdateStatus::Ok
        ));

        // Verify workload was removed from host
        let updated_host = api
            .host_collection
            .get_one_from(doc! { "_id": host_id })
            .await?;
        assert!(updated_host.is_some());

        let updated_host = updated_host.unwrap();
        assert!(updated_host.assigned_workloads.is_empty());

        // Clean up database
        mongod.cleanup().await?;
        Ok(())
    }
}
