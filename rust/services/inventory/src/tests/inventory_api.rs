#[cfg(test)]
mod tests {
    use crate::InventoryServiceApi;
    use anyhow::Result;
    use bson::doc;
    use bson::oid::ObjectId;
    use db_utils::schemas::host::Host;
    use db_utils::{mongodb::api::MongoDbAPI, schemas::workload::Workload};
    use mock_utils::{mongodb_runner::MongodRunner, nats_message::NatsMessage};
    use std::sync::Arc;

    // #[tokio::test]
    // async fn test_handle_authenticated_inventory_update() -> Result<()> {
    //     let mongod = MongodRunner::run().expect("Failed to run mongod");
    //     let db_client = mongod.client().expect("Failed to create db client");
    //     let api = InventoryServiceApi::new(&db_client).await?;

    //     // Create a host id to reference in the workload collection
    //     let host_id = ObjectId::new();

    //     // Create workload with specific requirements
    //     let workload = Workload::default();
    //     let workload_id = api
    //         .workload_collection
    //         .insert_one_into(workload)
    //         .await
    //         .expect("Failed to insert workload");

    //     // Create initial host with reference to workload (id) created above
    //     let host = Host {
    //         _id: Some(host_id),
    //         device_id: "mock_pubkey".to_string(),
    //         ..Default::default()
    //     };
    //     api.host_collection
    //         .insert_one_into(host.clone())
    //         .await
    //         .expect("Failed to insert host");

    //     // Test that inventory update still meets workload requirements
    //     let inventory_update = create_mock_inventory(Some(2000), Some(3), Some(20));
    //     let msg_payload = serde_json::to_vec(&inventory_update).unwrap();
    //     let msg = Arc::new(
    //         NatsMessage::new(format!("INVENTORY.{}.update", host.device_id), msg_payload)
    //             .into_message(),
    //     );

    //     let result = api.handle_host_inventory_update(msg).await?;
    //     assert!(matches!(
    //         result.status,
    //         crate::types::InventoryUpdateStatus::Ok
    //     ));

    //     // Verify host inventory was updated in db
    //     let updated_host = api
    //         .host_collection
    //         .get_one_from(doc! { "_id": host_id })
    //         .await?
    //         .expect("Failed to fetch updated host");
    //     assert_eq!(updated_host.assigned_workloads.len(), 1);
    //     assert!(updated_host.assigned_workloads.contains(&workload_id));

    //     Ok(())
    // }

    // #[tokio::test]
    // async fn test_handle_inventory_update_with_insufficient_resources() -> Result<()> {
    //     let mongod = MongodRunner::run().expect("Failed to run mongod");
    //     let db_client = mongod.client().expect("Failed to create db client");
    //     let api = InventoryServiceApi::new(&db_client)
    //         .await
    //         .expect("Failed to create api");

    //     // Create a host id to reference in the workload collection
    //     let host_id = ObjectId::new();

    //     // Create workload with specific requirements
    //     let workload = create_test_workload(
    //         None,
    //         Some(vec![host_id]),
    //         Some(1),
    //         Some(Capacity {
    //             drive: 500,
    //             cores: 16,
    //         }),
    //         Some(100),
    //         Some(0.9),
    //     );
    //     let workload_id = api
    //         .workload_collection
    //         .insert_one_into(workload)
    //         .await
    //         .expect("Failed to insert workload");

    //     // Create initial host with reference to workload (id) created above
    //     let initial_inventory = create_mock_inventory(Some(1000), Some(3), Some(20));
    //     let host = Host {
    //         _id: Some(host_id),
    //         device_id: "mock_pubkey".to_string(),
    //         assigned_workloads: vec![workload_id],
    //         inventory: initial_inventory.clone(),
    //         ..Default::default()
    //     };
    //     api.host_collection.insert_one_into(host.clone()).await?;

    //     // Test inventory update with insufficient resources
    //     let insufficient_inventory = create_mock_inventory(Some(100), Some(1), Some(4));

    //     let msg_payload = serde_json::to_vec(&insufficient_inventory).unwrap();

    //     let msg = Arc::new(
    //         NatsMessage::new(format!("INVENTORY.{}.update", host.device_id), msg_payload)
    //             .into_message(),
    //     );

    //     let result = api.handle_host_inventory_update(msg).await?;
    //     assert!(matches!(
    //         result.status,
    //         crate::types::InventoryUpdateStatus::Ok
    //     ));

    //     // Verify workload was removed from host
    //     let updated_host = api
    //         .host_collection
    //         .get_one_from(doc! { "_id": host_id })
    //         .await?
    //         .unwrap();
    //     assert!(updated_host.assigned_workloads.is_empty());

    //     Ok(())
    // }
}
