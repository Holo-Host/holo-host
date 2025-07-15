use nats_utils::types::{EndpointTraits, GetHeaderMap, GetResponse, GetSubjectTags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Test response for a jetstream service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResponse {
    pub message: String,
}

impl GetSubjectTags for TestResponse {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl GetResponse for TestResponse {
    fn get_response(&self) -> bytes::Bytes {
        serde_json::to_vec(&self).unwrap().into()
    }
}

impl GetHeaderMap for TestResponse {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        None
    }
}

impl EndpointTraits for TestResponse {}
