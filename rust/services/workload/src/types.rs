use std::collections::HashMap;

use util_libs::{db::schemas::WorkloadStatus, js_stream_service::{CreateTag, EndpointTraits}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResult (pub WorkloadStatus, pub Option<HashMap<String, String>>);
impl EndpointTraits for ApiResult {}
impl CreateTag for ApiResult {
    fn get_tags(&self) -> HashMap<String, String> {
        self.1.clone().unwrap_or_default()
    }
}
