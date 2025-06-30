pub mod jetstream_client;
pub mod jetstream_service;
// pub mod test_nats_server;

#[cfg(feature = "tests_integration_nats")]
pub mod leaf_server;

use crate::types::{CreateResponse, CreateTag, EndpointTraits};
use mock_utils::service_test_response::TestResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Wrapper type for TestResponse that implements the required traits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTestResponse(pub TestResponse);

impl CreateTag for LocalTestResponse {
    fn get_tags(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

impl CreateResponse for LocalTestResponse {
    fn get_response(&self) -> bytes::Bytes {
        serde_json::to_vec(&self.0).unwrap().into()
    }
}

impl EndpointTraits for LocalTestResponse {}
