use anyhow::Result;
use async_nats::Message;
use bson::{oid::ObjectId, DateTime};
use mongodb::Client;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::{path::PathBuf, str::FromStr, sync::Arc};
use tempfile::TempDir;
use util_libs::db::schemas::{
    self, Capacity, Host, Metadata, SystemSpecs, Workload, WorkloadState, WorkloadStatus,
};

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
    ]);

    let _child = cmd.spawn().expect("Failed to spawn mongod");

    let server_address = mongodb::options::ServerAddress::Unix {
        path: PathBuf::from_str(&socket_path).unwrap(),
    };
    let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
    let client = Client::with_options(client_options).unwrap();

    (client, tempdir)
}

// Helper function to create a test workload
pub fn create_test_workload() -> schemas::Workload {
    schemas::Workload {
        _id: Some(ObjectId::new()),
        metadata: schemas::Metadata {
            is_deleted: false,
            created_at: Some(DateTime::now()),
            updated_at: Some(DateTime::now()),
            deleted_at: None,
        },
        assigned_developer: ObjectId::new(),
        version: "0.1.0".to_string(),
        nix_pkg: "test-package".to_string(),
        min_hosts: 1,
        system_specs: schemas::SystemSpecs {
            capacity: schemas::Capacity {
                memory: 8,
                disk: 100,
                cores: 4,
            },
            avg_network_speed: 100,
            avg_uptime: 0.99,
        },
        assigned_hosts: vec![],
        status: schemas::WorkloadStatus {
            id: None,
            desired: schemas::WorkloadState::Running,
            actual: schemas::WorkloadState::Reported,
        },
    }
}

// Helper function to create a test host
pub fn create_test_host(
    device_id: Option<String>,
    assigned_hoster: Option<ObjectId>,
    assigned_workloads: Option<Vec<ObjectId>>,
    remaining_capacity: Option<Capacity>,
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
    host
}
