use anyhow::Result;
use async_nats::Message;
use async_nats::{
    jetstream::{self, Context},
    Client, ConnectOptions,
};
use bson::doc;
use bson::{oid::ObjectId, DateTime};
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::sync::Arc;
use std::{path::PathBuf, process::Stdio, str::FromStr};
use tempfile::TempDir;
use tokio::time::{sleep, Duration};
use util_libs::db::mongodb::MongoCollection;
use util_libs::db::schemas::{self, Capacity};

pub mod orchestrator_api;

pub struct TestMessage {
    subject: String,
    reply: Option<String>,
    payload: Vec<u8>,
}

impl TestMessage {
    pub fn new(subject: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            subject: subject.into(),
            reply: None,
            payload: payload.into(),
        }
    }

    pub fn into_message(self) -> Message {
        Message {
            subject: self.subject.into(),
            reply: self.reply.map(|r| r.into()),
            payload: self.payload.clone().into(),
            headers: None,
            status: None,
            description: None,
            length: self.payload.len(),
        }
    }
}

// Helper function to create a test MongoDB instance
pub async fn setup_test_db() -> (MongoDBClient, TempDir) {
    let tempdir = TempDir::new().unwrap();
    let socket_path = format!(
        "{}/mongod.sock",
        tempdir
            .path()
            .canonicalize()
            .unwrap()
            .as_mut_os_str()
            .to_str()
            .unwrap()
    );

    std::fs::File::create_new(&socket_path).unwrap();

    let mut cmd = std::process::Command::new("mongod");
    cmd.args([
        "--unixSocketPrefix",
        &tempdir.path().to_string_lossy(),
        "--dbpath",
        &tempdir.path().to_string_lossy(),
        "--bind_ip",
        &socket_path,
        "--port",
        "0",
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::null());

    let _child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn {cmd:?}: {e}"));

    let server_address = mongodb::options::ServerAddress::Unix {
        path: PathBuf::from_str(&socket_path).unwrap(),
    };

    let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
    let client = MongoDBClient::with_options(client_options).unwrap();
    log::debug!("setup_test_db client created");
    (client, tempdir)
}

// Helper function to create a test workload
pub fn create_test_workload_default() -> schemas::Workload {
    create_test_workload(None, None, None, None, None, None)
}

pub fn create_test_workload(
    assigned_developer: Option<ObjectId>,
    assigned_hosts: Option<Vec<ObjectId>>,
    min_hosts: Option<i32>,
    needed_capacity: Option<Capacity>,
    avg_network_speed: Option<i64>,
    avg_uptime: Option<f64>,
) -> schemas::Workload {
    let mut workload = schemas::Workload::default();
    if let Some(assigned_developer) = assigned_developer {
        workload.assigned_developer = assigned_developer;
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

// Helper function to create a test host
pub fn create_test_host(
    device_id: Option<String>,
    assigned_hoster: Option<ObjectId>,
    assigned_workloads: Option<Vec<ObjectId>>,
    remaining_capacity: Option<Capacity>,
    avg_network_speed: Option<i64>,
    avg_uptime: Option<f64>,
) -> schemas::Host {
    let mut host = schemas::Host::default();
    if let Some(device_id) = device_id {
        host.device_id = device_id;
    }
    if let Some(assigned_hoster) = assigned_hoster {
        host.assigned_hoster = assigned_hoster;
    }
    if let Some(assigned_workloads) = assigned_workloads {
        host.assigned_workloads = assigned_workloads;
    }
    if let Some(remaining_capacity) = remaining_capacity {
        host.remaining_capacity = remaining_capacity;
    }
    if let Some(avg_network_speed) = avg_network_speed {
        host.avg_network_speed = avg_network_speed;
    }
    if let Some(avg_uptime) = avg_uptime {
        host.avg_uptime = avg_uptime;
    }
    host
}
