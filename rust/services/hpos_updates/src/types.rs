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
pub struct HostUpdateResponseInfo {
    pub info: String,
    // pub host_id: ObjectId,
    // pub hoster_id: ObjectId,
    #[serde(flatten)]
    pub request_info: HostUpdateRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostUpdateResult {
    Success(HostUpdateResponseInfo),
    Error(HostUpdateResponseInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpdateApiResult {
    pub result: HostUpdateResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_response_tags: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maybe_headers: Option<async_nats::HeaderMap>,
}
impl EndpointTraits for HostUpdateApiResult {}
impl GetSubjectTags for HostUpdateApiResult {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl GetResponse for HostUpdateApiResult {
    fn get_response(&self) -> bytes::Bytes {
        let r = self.result.clone();
        match serde_json::to_vec(&r) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
impl GetHeaderMap for HostUpdateApiResult {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        self.maybe_headers.clone()
    }
}
