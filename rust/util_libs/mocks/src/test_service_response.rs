use nats_utils::types::{CreateResponse, CreateTag, EndpointTraits};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Test response type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResponse {
    pub message: String,
}

impl EndpointTraits for TestResponse {}

impl CreateTag for TestResponse {
    fn get_tags(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl CreateResponse for TestResponse {
    fn get_response(&self) -> bytes::Bytes {
        serde_json::to_vec(&self).unwrap().into()
    }
}
