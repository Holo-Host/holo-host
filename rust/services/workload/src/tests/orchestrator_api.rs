#![allow(dead_code)]
#![allow(unused_imports)]

use super::{
    create_test_host, create_test_workload, create_test_workload_default, MongodRunner, TestMessage,
};
use crate::{orchestrator_api::OrchestratorWorkloadApi, types::WorkloadResult};
use anyhow::Result;
use bson::doc;
use std::sync::Arc;
use util_libs::db::schemas::{WorkloadState, WorkloadStatus};

#[cfg(not(target_arch = "aarch64"))]
#[cfg(test)]
mod tests {
    use util_libs::db::{mongodb::MongoDbAPI, schemas::Capacity};

    use super::*;

    #[tokio::test]
    async fn test_add_workload() -> Result<()> {
        let mongod = MongodRunner::run().unwrap();
        let db_client = mongod.client().unwrap();

        let api = OrchestratorWorkloadApi::new(&db_client).await?;
        let workload = create_test_workload_default();
        let msg_payload = serde_json::to_vec(&workload).unwrap();
        let msg = Arc::new(TestMessage::new("WORKLOAD.add", msg_payload).into_message());
        let result = api.add_workload(msg).await?;

        assert!(result.result.status.id.is_some());
        assert!(matches!(
            result.result.status.actual,
            WorkloadState::Reported
        ));
        assert!(matches!(
            result.result.status.desired,
            WorkloadState::Running
        ));
        Ok(())
    }

    #[tokio::test]
    async fn test_update_workload() -> Result<()> {
        let mongod = MongodRunner::run().unwrap();
        let db_client = mongod.client().unwrap();

        let api = OrchestratorWorkloadApi::new(&db_client).await?;

        // First add a workload
        let mut workload = create_test_workload_default();
        let workload_id = api
            .workload_collection
            .insert_one_into(workload.clone())
            .await?;
        workload._id = Some(workload_id);

        // Then update it
        let msg_payload = serde_json::to_vec(&workload).unwrap();
        let msg = Arc::new(TestMessage::new("WORKLOAD.update", msg_payload).into_message());

        let result = api.update_workload(msg).await?;

        assert!(matches!(
            result.result.status.actual,
            WorkloadState::Updating
        ));
        assert!(matches!(
            result.result.status.desired,
            WorkloadState::Updated
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_workload() -> Result<()> {
        let mongod = MongodRunner::run().unwrap();
        let db_client = mongod.client().unwrap();

        let api = OrchestratorWorkloadApi::new(&db_client).await?;

        // First add a workload
        let workload = create_test_workload_default();
        let workload_id = api.workload_collection.insert_one_into(workload).await?;

        // Then remove it
        let msg_payload = serde_json::to_vec(&workload_id).unwrap();
        let msg = Arc::new(TestMessage::new("WORKLOAD.remove", msg_payload).into_message());

        let result = api.remove_workload(msg).await?;

        assert!(matches!(
            result.result.status.actual,
            WorkloadState::Removed
        ));
        assert!(matches!(
            result.result.status.desired,
            WorkloadState::Uninstalled
        ));

        // Verify workload is marked as deleted
        let removed_workload = api
            .workload_collection
            .get_one_from(doc! { "_id": workload_id })
            .await?
            .unwrap();
        assert!(removed_workload.metadata.is_deleted);
        assert!(removed_workload.metadata.deleted_at.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_verify_host_meets_workload_criteria() -> Result<()> {
        let mongod = MongodRunner::run().unwrap();
        let db_client = mongod.client().unwrap();

        let api = OrchestratorWorkloadApi::new(&db_client).await?;
        let required_avg_network_speed = 100;
        let required_avg_uptime = 0.85;
        let required_capacity = Capacity {
            memory: 8,
            disk: 100,
            cores: 4,
        };
        let valid_host_remaining_capacity = Capacity {
            memory: 8,
            disk: 100,
            cores: 4,
        };

        let workload = create_test_workload(
            None,
            None,
            Some(1),
            Some(required_capacity),
            Some(required_avg_network_speed),
            Some(required_avg_uptime),
        );
        let host = create_test_host(
            None,
            None,
            None,
            Some(valid_host_remaining_capacity),
            Some(required_avg_network_speed),
            Some(required_avg_uptime),
        );

        // Test when host meets criteria
        assert!(api.verify_host_meets_workload_criteria(&host, &workload));

        // Test when host memorydoesn't meet memory criteria
        let mut ineligible_host = host.clone();
        ineligible_host.remaining_capacity.memory = 4; // Less than workload requirement
        assert!(!api.verify_host_meets_workload_criteria(&ineligible_host, &workload));

        // Test when host disk space doesn't meet disk criteria
        let mut ineligible_host = host.clone();
        ineligible_host.remaining_capacity.disk = 50; // Less than workload requirement
        assert!(!api.verify_host_meets_workload_criteria(&ineligible_host, &workload));

        // Test when host cores count doesn't meet cores criteria
        let mut ineligible_host = host.clone();
        ineligible_host.remaining_capacity.cores = 2; // Less than workload requirement
        assert!(!api.verify_host_meets_workload_criteria(&ineligible_host, &workload));

        Ok(())
    }

    #[tokio::test]
    async fn test_handle_db_insertion() -> Result<()> {
        let mongod = MongodRunner::run().unwrap();
        let db_client = mongod.client().unwrap();

        let api = OrchestratorWorkloadApi::new(&db_client).await?;
        let required_avg_network_speed = 500;
        let required_avg_uptime = 0.90;
        let required_capacity = Capacity {
            memory: 800,
            disk: 1000,
            cores: 20,
        };
        let valid_host_remaining_capacity = Capacity {
            memory: 800,
            disk: 1000,
            cores: 20,
        };

        // Create and add a host first
        let host = create_test_host(
            None,
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
        workload._id = Some(workload_id);

        let msg_payload = serde_json::to_vec(&workload).unwrap();
        let msg = Arc::new(TestMessage::new("WORKLOAD.insert", msg_payload).into_message());

        let result = api.handle_db_insertion(msg).await?;

        assert!(matches!(
            result.result.status.actual,
            WorkloadState::Assigned
        ));
        assert!(matches!(
            result.result.status.desired,
            WorkloadState::Running
        ));

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
        let mongod = MongodRunner::run().unwrap();
        let db_client = mongod.client().unwrap();

        let api = OrchestratorWorkloadApi::new(&db_client).await?;

        // Create and add a workload first
        let workload = create_test_workload_default();
        let workload_id = api.workload_collection.insert_one_into(workload).await?;

        // Create status update
        let status = WorkloadStatus {
            id: Some(workload_id),
            desired: WorkloadState::Running,
            actual: WorkloadState::Running,
        };
        let result = WorkloadResult {
            status: status.clone(),
            workload: None,
        };

        let msg_payload = serde_json::to_vec(&result).unwrap();
        let msg = Arc::new(TestMessage::new("WORKLOAD.status", msg_payload).into_message());

        let update_result = api.handle_status_update(msg).await?;

        assert!(matches!(
            update_result.result.status.actual,
            WorkloadState::Running
        ));
        assert!(matches!(
            update_result.result.status.desired,
            WorkloadState::Running
        ));

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
