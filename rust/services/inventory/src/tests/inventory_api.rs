#![allow(dead_code)]
#![allow(unused_imports)]

use super::*;
use crate::{types::InventoryPayloadType, InventoryServiceApi};
use bson::doc;
use std::sync::Arc;
use util_libs::db::mongodb::MongoDbAPI;

#[cfg(not(target_arch = "aarch64"))]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_authenticated_inventory_update() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run mongod");
        let db_client = mongod.client().expect("Failed to create db client");
        let api = InventoryServiceApi::new(&db_client).await?;

        // Create a host id to reference in the workload collection
        let host_id = ObjectId::new();

        // Create workload with specific requirements
        let workload = create_mock_workload(
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
        let workload_id = api
            .workload_collection
            .insert_one_into(workload)
            .await
            .expect("Failed to insert workload");

        // Create initial host with reference to workload (id) created above
        let initial_inventory = create_mock_inventory(Some(1000), Some(3), Some(20));
        let host = schemas::Host {
            _id: Some(host_id),
            device_id: "mock_pubkey".to_string(),
            inventory: initial_inventory.clone(),
            assigned_workloads: vec![workload_id],
            ..Default::default()
        };
        api.host_collection
            .insert_one_into(host.clone())
            .await
            .expect("Failed to insert host");

        // Test that inventory update still meets workload requirements
        let inventory_update = create_mock_inventory(Some(2000), Some(3), Some(20));
        let payload = InventoryPayloadType::Authenticated(inventory_update);
        let msg_payload = serde_json::to_vec(&payload).unwrap();
        let msg = Arc::new(
            TestMessage::new(
                format!("INVENTORY.authenticated.{}.update", host.device_id),
                msg_payload,
            )
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

        Ok(())
    }

    #[tokio::test]
    async fn test_handle_unauthenticated_inventory_update() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run mongod");
        let db_client = mongod.client().expect("Failed to create db client");
        let api = InventoryServiceApi::new(&db_client)
            .await
            .expect("Failed to create api");

        let host_id = ObjectId::new();

        // Create initial host
        let initial_inventory = create_mock_inventory(Some(1000), Some(3), Some(20));
        let host = schemas::Host {
            _id: Some(host_id),
            device_id: "mock_pubkey".to_string(),
            inventory: initial_inventory.clone(),
            ..Default::default()
        };
        api.host_collection
            .insert_one_into(host.clone())
            .await
            .expect("Failed to insert host");

        // Test unauthenticated inventory update
        let new_inventory = create_mock_inventory(Some(2000), Some(4), Some(24));
        let payload = InventoryPayloadType::Unauthenticated(new_inventory.clone());
        let msg_payload = serde_json::to_vec(&payload).unwrap();
        let msg = Arc::new(
            TestMessage::new(
                format!("INVENTORY.unauthenticated.{}.update", host.device_id),
                msg_payload,
            )
            .into_message(),
        );

        let result = api.handle_host_inventory_update(msg).await?;
        assert!(matches!(
            result.status,
            crate::types::InventoryUpdateStatus::Ok
        ));

        // Verify host inventory was updated
        let updated_host = api
            .host_collection
            .get_one_from(doc! { "_id": host_id })
            .await?
            .unwrap();
        assert_eq!(
            updated_host.inventory.drives.len(),
            new_inventory.drives.len()
        );
        assert_eq!(updated_host.inventory.cpus.len(), new_inventory.cpus.len());

        Ok(())
    }

    #[tokio::test]
    async fn test_handle_inventory_update_with_insufficient_resources() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run mongod");
        let db_client = mongod.client().expect("Failed to create db client");
        let api = InventoryServiceApi::new(&db_client)
            .await
            .expect("Failed to create api");

        // Create a host id to reference in the workload collection
        let host_id = ObjectId::new();

        // Create workload with specific requirements
        let workload = create_mock_workload(
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
        let workload_id = api
            .workload_collection
            .insert_one_into(workload)
            .await
            .expect("Failed to insert workload");

        // Create initial host with reference to workload (id) created above
        let initial_inventory = create_mock_inventory(Some(1000), Some(3), Some(20));
        let host = schemas::Host {
            _id: Some(host_id),
            device_id: "mock_pubkey".to_string(),
            assigned_workloads: vec![workload_id],
            inventory: initial_inventory.clone(),
            ..Default::default()
        };
        api.host_collection.insert_one_into(host.clone()).await?;

        // Test inventory update with insufficient resources
        let insufficient_inventory = create_mock_inventory(Some(100), Some(1), Some(4));
        let payload = InventoryPayloadType::Authenticated(insufficient_inventory);
        let msg_payload = serde_json::to_vec(&payload).unwrap();
        let msg = Arc::new(
            TestMessage::new(
                format!("INVENTORY.authenticated.{}.update", host.device_id),
                msg_payload,
            )
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
            .await?
            .unwrap();
        assert!(updated_host.assigned_workloads.is_empty());

        Ok(())
    }
}
