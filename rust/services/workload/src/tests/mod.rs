use anyhow::Context;
use async_nats::Message;
use bson::oid::ObjectId;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use std::{path::PathBuf, process::Stdio, str::FromStr};
use tempfile::TempDir;
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

/// This module implements running ephemeral Mongod instances.
/// It disables TCP and relies only unix domain sockets.
pub struct MongodRunner {
    _child: std::process::Child,

    // this is stored to prevent premature removing of the tempdir
    tempdir: TempDir,
}

impl MongodRunner {
    fn socket_path(tempdir: &TempDir) -> anyhow::Result<String> {
        Ok(format!(
            "{}/mongod.sock",
            tempdir
                .path()
                .canonicalize()?
                .as_mut_os_str()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("can't convert path to str"))?
        ))
    }

    pub fn run() -> anyhow::Result<Self> {
        let tempdir = TempDir::new().unwrap();
        std::fs::File::create_new(Self::socket_path(&tempdir)?)?;

        let mut cmd = std::process::Command::new("mongod");
        cmd.args([
            "--unixSocketPrefix",
            &tempdir.path().to_string_lossy(),
            "--dbpath",
            &tempdir.path().to_string_lossy(),
            "--bind_ip",
            &Self::socket_path(&tempdir)?,
            "--port",
            &0.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

        let child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to spawn {cmd:?}: {e}"));

        let new_self = Self {
            _child: child,
            tempdir,
        };

        std::fs::exists(Self::socket_path(&new_self.tempdir)?)
            .context("mongod socket should exist")?;

        println!(
            "MongoDB Server is running at {:?}",
            new_self.socket_pathbuf()
        );

        Ok(new_self)
    }

    fn socket_pathbuf(&self) -> anyhow::Result<PathBuf> {
        Ok(PathBuf::from_str(&Self::socket_path(&self.tempdir)?)?)
    }

    pub fn client(&self) -> anyhow::Result<MongoDBClient> {
        let server_address = mongodb::options::ServerAddress::Unix {
            path: self.socket_pathbuf()?,
        };
        let client_options = ClientOptions::builder().hosts(vec![server_address]).build();
        Ok(MongoDBClient::with_options(client_options)?)
    }
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
