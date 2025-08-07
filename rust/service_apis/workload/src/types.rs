use bson::oid::ObjectId;
use db_utils::schemas::workload::{Workload, WorkloadStatus};
use nats_utils::types::{EndpointTraits, GetHeaderMap, GetResponse, GetSubjectTags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::AsRefStr;
use thiserror::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HostIdJSON {
    pub _id: ObjectId,
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, AsRefStr, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum WorkloadServiceSubjects {
    Add,
    Update,
    Delete,
    Insert, // db change stream trigger
    Modify, // db change stream trigger
    HandleStatusUpdate,
    SendStatus,
    Install,
    Uninstall,
    // /// TODO: Command replaces Add, Update, Delete, Install, Uninstall, SendStatus
    Command,
}

// Shared workload error type that can be used by both service API and clients
#[derive(Debug, Error)]
pub enum WorkloadError {
    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O operation failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("NATS operation failed: {0}")]
    Nats(String),

    #[error("Jetstream operation failed: {0}")]
    Jetstream(String),

    #[error("Key-value store operation failed: {0}")]
    KeyValue(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Workload management failed: {0}")]
    WorkloadManagement(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Service error: {0}")]
    Service(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

impl WorkloadError {
    pub fn nats_failed(msg: &str) -> Self {
        Self::Nats(msg.to_string())
    }

    pub fn jetstream_failed(msg: &str) -> Self {
        Self::Jetstream(msg.to_string())
    }

    pub fn kv_failed(msg: &str) -> Self {
        Self::KeyValue(msg.to_string())
    }

    pub fn workload_failed(msg: &str) -> Self {
        Self::WorkloadManagement(msg.to_string())
    }

    pub fn config_error(msg: &str) -> Self {
        Self::Configuration(msg.to_string())
    }

    pub fn service_error(msg: &str) -> Self {
        Self::Service(msg.to_string())
    }

    pub fn invalid_state(msg: &str) -> Self {
        Self::InvalidState(msg.to_string())
    }
}

pub type WorkloadOpResult<T> = Result<T, WorkloadError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadResult {
    Status(WorkloadStatus),
    Workload(Workload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadApiResult {
    pub result: WorkloadResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_response_tags: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_headers: Option<async_nats::HeaderMap>,
}
impl EndpointTraits for WorkloadApiResult {}
impl GetSubjectTags for WorkloadApiResult {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl GetResponse for WorkloadApiResult {
    fn get_response(&self) -> bytes::Bytes {
        let r = self.result.clone();
        match serde_json::to_vec(&r) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
impl GetHeaderMap for WorkloadApiResult {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        self.maybe_headers.clone()
    }
}
