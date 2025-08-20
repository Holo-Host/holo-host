#![allow(dead_code)]

use bson::oid::ObjectId;
use db_utils::schemas;

// Helper function to create a test workload
pub fn create_test_workload_default(
    owner_id: Option<ObjectId>,
    manifest_id: Option<ObjectId>,
) -> schemas::workload::Workload {
    let mock_owner_id = owner_id.unwrap_or_default();
    let mock_manifest_id = manifest_id.unwrap_or_default();
    create_test_workload(mock_owner_id, mock_manifest_id, None, None, None, None)
}

pub fn create_test_workload(
    owner: ObjectId, // previously called "assigned_developer"
    manifest: ObjectId,
    min_hosts: Option<i32>,
    regions: Option<Vec<String>>,
    jurisdictions: Option<Vec<String>>,
    context: Option<schemas::workload::Context>, // assigned_hosts: Option<Vec<ObjectId>>,
) -> schemas::workload::Workload {
    let mut workload = schemas::workload::Workload::new(owner, manifest);
    if let Some(min_hosts) = min_hosts {
        workload.execution_policy.instances = min_hosts;
    }
    if let Some(jurisdictions) = jurisdictions {
        workload.execution_policy.jurisdictions = jurisdictions;
    }
    if let Some(regions) = regions {
        workload.execution_policy.regions = regions;
    }
    if let Some(context) = context {
        workload.context = context;
    }
    // if let Some(assigned_hosts) = assigned_hosts {
    //     workload.assigned_hosts = assigned_hosts;
    // }
    workload
}
