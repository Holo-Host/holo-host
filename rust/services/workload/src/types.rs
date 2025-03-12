use bson::oid::ObjectId;
use db_utils::schemas::{self, WorkloadStatus};
use nats_utils::types::{CreateResponse, CreateTag, EndpointTraits};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::AsRefStr;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HostIdJSON {
    pub _id: ObjectId,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, AsRefStr)]
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
    UpdateInstalled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadResult {
    pub status: WorkloadStatus,
    pub workload: Option<schemas::Workload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadApiResult {
    pub result: WorkloadResult,
    pub maybe_response_tags: Option<HashMap<String, String>>,
}
impl EndpointTraits for WorkloadApiResult {}
impl CreateTag for WorkloadApiResult {
    fn get_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl CreateResponse for WorkloadApiResult {
    fn get_response(&self) -> bytes::Bytes {
        let r = self.result.clone();
        match serde_json::to_vec(&r) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
