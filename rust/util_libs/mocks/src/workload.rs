#![allow(dead_code)]

use bson::oid::ObjectId;
use db_utils::schemas::{self, workload::Capacity};

// Helper function to create a test workload
pub fn create_test_workload_default() -> schemas::workload::Workload {
    create_test_workload(None, None, None, None, None, None)
}

pub fn create_test_workload(
    assigned_developer: Option<ObjectId>,
    assigned_hosts: Option<Vec<ObjectId>>,
    min_hosts: Option<i32>,
    needed_capacity: Option<Capacity>,
    avg_network_speed: Option<i32>,
    avg_uptime: Option<f32>,
) -> schemas::workload::Workload {
    let mut workload = schemas::workload::Workload::default();
    if let Some(assigned_developer) = assigned_developer {
        workload.owner = assigned_developer;
    }
    if let Some(assigned_hosts) = assigned_hosts {
        workload.assigned_hosts = assigned_hosts;
    }
    if let Some(min_hosts) = min_hosts {
        workload.min_hosts = min_hosts;
    }
    if let Some(needed_capacity) = needed_capacity {
        workload.system_specs.capacity = needed_capacity;
    }
    if let Some(avg_network_speed) = avg_network_speed {
        workload.system_specs.avg_network_speed = avg_network_speed;
    }
    if let Some(avg_uptime) = avg_uptime {
        workload.system_specs.avg_uptime = avg_uptime;
    }
    workload
}
