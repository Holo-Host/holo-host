use util_libs::{db::schemas::WorkloadStatus, js_stream_service::{CreateTag, EndpointTraits}};
use serde::{Deserialize, Serialize};

pub use String as WorkloadId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResult (pub Option<WorkloadId>, pub WorkloadStatus, pub Option<Vec<String>>);

impl CreateTag for ApiResult {
    fn get_tags(&self) -> Option<Vec<String>> {
        self.2.clone()
    }
}

impl EndpointTraits for ApiResult {}