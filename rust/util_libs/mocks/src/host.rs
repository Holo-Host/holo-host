#![allow(dead_code)]

use bson::oid::ObjectId;
use db_utils::schemas;
use hpos_hal::inventory::{HoloDriveInventory, HoloInventory, HoloProcessorInventory};

// Helper function to create a list of mock holo processors in bulk
pub fn gen_mock_processors(max_processors: i64) -> Vec<HoloProcessorInventory> {
    let mut mock_holo_processors = vec![];
    for _i in 0..max_processors {
        mock_holo_processors.push(HoloProcessorInventory::default());
    }
    mock_holo_processors
}

// Helper function to create a test host inventory
pub fn create_mock_inventory(
    drive_capacity: Option<u64>,
    num_drives: Option<usize>,
    num_processors: Option<i64>,
) -> HoloInventory {
    let mut inventory = HoloInventory::default();

    let drive_capacity = drive_capacity.unwrap_or_default();
    let mock_drive = HoloDriveInventory {
        capacity_bytes: Some(drive_capacity),
        ..Default::default()
    };

    let num_drives = num_drives.unwrap_or_default();
    inventory.drives = vec![mock_drive; num_drives];

    let num_processors = num_processors.unwrap_or_default();
    inventory.cpus = gen_mock_processors(num_processors);

    inventory
}

// Helper function to create a test host
pub fn create_test_host(
    device_id: Option<String>,
    assigned_hoster: Option<ObjectId>,
    assigned_workloads: Option<Vec<ObjectId>>,
    holo_inventory: Option<HoloInventory>,
    avg_network_speed: Option<i64>,
    avg_uptime: Option<f64>,
) -> schemas::Host {
    let mut host = schemas::Host::default();
    if let Some(device_id) = device_id {
        host.device_id = device_id;
    }
    if let Some(assigned_hoster) = assigned_hoster {
        host.assigned_hoster = Some(assigned_hoster);
    }
    if let Some(assigned_workloads) = assigned_workloads {
        host.assigned_workloads = assigned_workloads;
    }
    if let Some(holo_inventory) = holo_inventory {
        host.inventory = holo_inventory;
    }
    if let Some(avg_network_speed) = avg_network_speed {
        host.avg_network_speed = avg_network_speed;
    }
    if let Some(avg_uptime) = avg_uptime {
        host.avg_uptime = avg_uptime;
    }
    host
}
