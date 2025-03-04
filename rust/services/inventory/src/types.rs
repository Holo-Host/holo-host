use hpos_hal::inventory::HoloInventory;
use nats_utils::types::{CreateResponse, CreateTag, EndpointTraits};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InventoryPayloadType {
    Authenticated(HoloInventory),
    Unauthenticated(HoloInventory),
}

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
impl CreateTag for InventoryApiResult {
    fn get_tags(&self) -> HashMap<String, String> {
        self.maybe_response_tags.clone().unwrap_or_default()
    }
}
impl CreateResponse for InventoryApiResult {
    fn get_response(&self) -> bytes::Bytes {
        let s = self.status.clone();
        match serde_json::to_vec(&s) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}
