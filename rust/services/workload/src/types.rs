use bson::oid::ObjectId;
use db_utils::schemas::workload::{Workload, WorkloadStatus};
use nats_utils::types::{EndpointTraits, GetHeaderMap, GetResponse, GetSubjectTags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::AsRefStr;

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

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct WorkloadResult {
//     pub status: WorkloadStatus,
//     pub workload: Option<Workload>,
// }

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
