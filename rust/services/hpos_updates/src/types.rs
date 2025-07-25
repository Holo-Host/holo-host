use nats_utils::types::{EndpointTraits, GetHeaderMap, GetResponse, GetSubjectTags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// use bson::oid::ObjectId;
use strum_macros::AsRefStr;

#[derive(Serialize, Deserialize, Clone, Debug, AsRefStr, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum HostUpdateServiceSubjects {
    Update,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpdateRequest {
    pub channel: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostUpdateState {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpdateInfo {
    #[serde(flatten)]
    pub request_info: HostUpdateRequest,
    pub state: HostUpdateState,
    pub context: Option<String>,
    // pub host_id: ObjectId,
    // pub hoster_id: ObjectId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpdateApiRequest {
    pub info: HostUpdateInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_response_tags: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_headers: Option<async_nats::HeaderMap>,
}
impl EndpointTraits for HostUpdateApiRequest {}
impl GetSubjectTags for HostUpdateApiRequest {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl GetResponse for HostUpdateApiRequest {
    fn get_response(&self) -> bytes::Bytes {
        let r = self.info.clone();
        match serde_json::to_vec(&r) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
impl GetHeaderMap for HostUpdateApiRequest {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        self.maybe_headers.clone()
    }
}
