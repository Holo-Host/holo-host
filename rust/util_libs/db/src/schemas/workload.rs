use std::collections::HashMap;
use bson::oid::ObjectId;
use super::metadata::Metadata;

pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicyVisibility {
    #[default]
    Public,
    Private,
}

fn default_instances() -> i32 {
    1
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct ExecutionPolicy {
    /// the jurisdictions to deploy the workload in
    /// this maps to the jurisdiction code in the jurisdiction collection
    pub jurisdictions: Vec<String>,
    /// the region to deploy the workload in
    /// this maps to the region code in the region collection
    pub regions: Vec<String>,
    /// minimum number of instances required for this workload
    #[serde(default = "default_instances")]
    pub instances: i32,
    /// the visibility of the workload on hosts
    pub visibility: ExecutionPolicyVisibility,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct Workload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    /// the user that owns this resource (workload)
    pub owner: ObjectId,

    /// the execution policy for the workload
    pub execution_policy: ExecutionPolicy,

    /// bootstrap server url
    pub bootstrap_server_url: Option<String>,

    /// signal server url
    pub signal_server_url: Option<String>,

    /// network seed
    pub network_seed: Option<String>,

    /// membrane proof
    pub memproof: Option<HashMap<String, String>>,

    /// HTTP gateway enable flag
    pub http_gw_enable: bool,

    /// HTTP gateway allowed functions
    pub http_gw_allowed_fns: Option<Vec<String>>,
}