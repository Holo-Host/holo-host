use nats_utils::types::{EndpointTraits, GetHeaderMap, GetResponse, GetSubjectTags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InventoryUpdateStatus {
    Ok,
    Err(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryApiResult {
    pub status: InventoryUpdateStatus,
    pub maybe_response_tags: Option<HashMap<String, String>>,
}
impl EndpointTraits for InventoryApiResult {}
impl GetSubjectTags for InventoryApiResult {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl GetResponse for InventoryApiResult {
    fn get_response(&self) -> bytes::Bytes {
        let s = self.status.clone();
        match serde_json::to_vec(&s) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
impl GetHeaderMap for InventoryApiResult {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        None
    }
}
