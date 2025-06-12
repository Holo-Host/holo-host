#[cfg(test)]
mod tests {
    use crate::{orchestrator_api::OrchestratorWorkloadApi, types::WorkloadResult};
    use anyhow::Result;
    use bson::doc;
    use db_utils::schemas::workload::{
        Capacity, WorkloadState, WorkloadStatePayload, WorkloadStatus,
    };
    use hpos_hal::inventory::{HoloDriveInventory, HoloInventory};
    use mock_utils::{
        host::{create_test_host, gen_mock_processors},
        mongodb_runner::MongodRunner,
        nats_message::NatsMessage,
        workload::{create_test_workload, create_test_workload_default},
    };
    use std::sync::Arc;

    use db_utils::mongodb::api::MongoDbAPI;

    #[tokio::test]
    async fn test_add_workload() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");

        let api = OrchestratorWorkloadApi::new(&db_client).await?;
        let workload = create_test_workload_default();
        println!("workload: {:#?}", workload);
        let msg_payload = serde_json::to_vec(&workload).unwrap();
        let msg = Arc::new(NatsMessage::new("WORKLOAD.add", msg_payload).into_message());
        let r = api.add_workload(msg).await?;
        println!("workload result: {:#?}", r);

        if let WorkloadResult::Status(status) = r.result {
            assert!(status.id.is_some());
            assert!(matches!(status.actual, WorkloadState::Reported));
            assert!(matches!(status.desired, WorkloadState::Running));
        } else {
            panic!("Expected WorkloadResult::Status, got something else");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_workload() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");

        let api = OrchestratorWorkloadApi::new(&db_client).await?;

        // First add a workload
        let mut workload = create_test_workload_default();
        let workload_id = api
            .workload_collection
            .insert_one_into(workload.clone())
            .await?;
        workload._id = workload_id;

        // Then update it
        let msg_payload = serde_json::to_vec(&workload).unwrap();
        let msg = Arc::new(NatsMessage::new("WORKLOAD.update", msg_payload).into_message());

        let r = api.update_workload(msg).await?;

        if let WorkloadResult::Status(status) = r.result {
            assert!(matches!(status.actual, WorkloadState::Updated));
            assert!(matches!(status.desired, WorkloadState::Running));
        } else {
            panic!("Expected WorkloadResult::Status, got something else");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_workload() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");

        let api = OrchestratorWorkloadApi::new(&db_client).await?;

        // First add a workload
        let mut workload = create_test_workload_default();
        let workload_id = api
            .workload_collection
            .insert_one_into(workload.clone())
            .await?;
        workload._id = workload_id;

        // Then remove it
        let msg_payload = serde_json::to_vec(&workload).expect("Failed to serialize workload id");
        let msg = Arc::new(NatsMessage::new("WORKLOAD.delete", msg_payload).into_message());

        let r = api.delete_workload(msg).await?;

        if let WorkloadResult::Status(status) = r.result {
            assert!(matches!(status.actual, WorkloadState::Deleted));
            assert!(matches!(status.desired, WorkloadState::Uninstalled));
        } else {
            panic!("Expected WorkloadResult::Status, got something else");
        }

        // Verify workload is marked as deleted
        let deleted_workload = api
            .workload_collection
            .get_one_from(doc! { "_id": workload_id })
            .await?
            .unwrap();
        assert!(deleted_workload.metadata.is_deleted);
        assert!(deleted_workload.metadata.deleted_at.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_verify_host_meets_workload_criteria() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");

        let api = OrchestratorWorkloadApi::new(&db_client).await?;
        let required_avg_network_speed = 100;
        let required_avg_uptime = 0.85;
        let required_capacity = Capacity {
            drive: 200,
            cores: 18,
        };
        #[allow(clippy::field_reassign_with_default)]
        let mut valid_host_remaining_capacity = HoloInventory::default();
        let mut mock_holo_drive = HoloDriveInventory {
            capacity_bytes: Some(100),
            ..Default::default()
        };
        valid_host_remaining_capacity.drives = vec![
            mock_holo_drive.clone(),
            mock_holo_drive.clone(),
            mock_holo_drive.clone(),
        ];
        valid_host_remaining_capacity.cpus = gen_mock_processors(20);

        let workload = create_test_workload(
            None,
            None,
            Some(1),
            Some(required_capacity),
            Some(required_avg_network_speed),
            Some(required_avg_uptime),
        );

        let device_id = "host_inventory_machine_id_1";
        let host = create_test_host(
            device_id,
            None,
            None,
            Some(valid_host_remaining_capacity),
            Some(required_avg_network_speed),
            Some(required_avg_uptime),
        );

        // Test when host meets criteria
        assert!(api._verify_host_meets_workload_criteria(&host.inventory, &workload));

        // Test when host drive space doesn't meet disk criteria
        let mut ineligible_host = host.clone();
        // Create new holo drive with available capacity less than workload requirement
        mock_holo_drive.capacity_bytes = Some(0);
        ineligible_host.inventory.drives = vec![
            mock_holo_drive.clone(),
            mock_holo_drive.clone(),
            mock_holo_drive,
        ];
        assert!(!api._verify_host_meets_workload_criteria(&ineligible_host.inventory, &workload));

        // Test when host cores count doesn't meet cores criteria
        let mut ineligible_host = host.clone();
        ineligible_host.inventory.cpus = gen_mock_processors(14); // Less than workload requirement
        assert!(!api._verify_host_meets_workload_criteria(&ineligible_host.inventory, &workload));

        Ok(())
    }

    #[tokio::test]
    async fn test_manage_workload_on_host() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");

        let api = OrchestratorWorkloadApi::new(&db_client).await?;
        let required_avg_network_speed = 500;
        let required_avg_uptime = 0.90;
        let required_capacity = Capacity {
            drive: 1000,
            cores: 20,
        };
        #[allow(clippy::field_reassign_with_default)]
        let mut valid_host_remaining_capacity = HoloInventory::default();
        let mock_holo_drive = HoloDriveInventory {
            capacity_bytes: Some(100),
            ..Default::default()
        };
        valid_host_remaining_capacity.drives = vec![
            mock_holo_drive.clone(),
            mock_holo_drive.clone(),
            mock_holo_drive,
        ];
        valid_host_remaining_capacity.cpus = gen_mock_processors(20);

        // Create and add a host first
        let device_id = "host_inventory_machine_id_2";
        let host = create_test_host(
            device_id,
            None,
            None,
            Some(valid_host_remaining_capacity),
            Some(required_avg_network_speed),
            Some(required_avg_uptime),
        );
        let host_id = api.host_collection.insert_one_into(host).await?;

        // Create workload
        let mut workload = create_test_workload(
            None,
            None,
            Some(1),
            Some(required_capacity),
            Some(required_avg_network_speed),
            Some(required_avg_uptime),
        );

        let workload_id = api
            .workload_collection
            .insert_one_into(workload.clone())
            .await?;
        workload._id = workload_id;
        workload.status.desired = WorkloadState::Running;
        workload.status.actual = WorkloadState::Reported;

        let msg_payload = serde_json::to_vec(&workload).unwrap();
        let msg = Arc::new(NatsMessage::new("WORKLOAD.insert", msg_payload).into_message());

        let r = api.manage_workload_on_host(msg).await?;

        if let WorkloadResult::Workload(returned_workload) = r.result {
            assert!(matches!(
                returned_workload.status.actual,
                WorkloadState::Assigned
            ));
            assert_eq!(returned_workload.status.desired, workload.status.desired);
        } else {
            panic!("Expected WorkloadResult::Workload, got something else");
        }

        // Verify host assignment
        let updated_host = api
            .host_collection
            .get_one_from(doc! { "_id": host_id })
            .await?
            .unwrap();

        assert!(updated_host.assigned_workloads.contains(&workload_id));
        Ok(())
    }

    #[tokio::test]
    async fn test_handle_status_update() -> Result<()> {
        let mongod = MongodRunner::run().expect("Failed to run Mongodb Runner");
        let db_client = mongod
            .client()
            .expect("Failed to connect client to Mongodb");

        let api = OrchestratorWorkloadApi::new(&db_client).await?;

        // Create and add a workload first
        let workload = create_test_workload_default();
        let workload_id = api.workload_collection.insert_one_into(workload).await?;

        // Create status update
        let status = WorkloadStatus {
            id: Some(workload_id),
            desired: WorkloadState::Running,
            actual: WorkloadState::Running,
            payload: WorkloadStatePayload::None,
        };
        let result = WorkloadResult::Status(status.clone());

        let msg_payload = serde_json::to_vec(&result).unwrap();
        let msg = Arc::new(NatsMessage::new("WORKLOAD.status", msg_payload).into_message());

        let r = api.handle_status_update(msg).await?;

        if let WorkloadResult::Status(status) = r.result {
            assert!(matches!(status.actual, WorkloadState::Running));
            assert!(matches!(status.desired, WorkloadState::Running));
        } else {
            panic!("Expected WorkloadResult::Status, got something else");
        }

        // Verify workload status was updated
        let updated_workload = api
            .workload_collection
            .get_one_from(doc! { "_id": workload_id })
            .await?
            .unwrap();
        assert!(matches!(
            updated_workload.status.actual,
            WorkloadState::Running
        ));
        assert!(matches!(
            updated_workload.status.desired,
            WorkloadState::Running
        ));

        Ok(())
    }
}
