use util_libs::db::schemas::WorkloadStatus;
use serde::{Deserialize, Serialize};

pub use String as WorkloadId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResult (pub Option<WorkloadId>, pub WorkloadStatus);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadPayloadTypes {
    Workload,
    MongoDbId
}