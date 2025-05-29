use crate::mongodb::traits::{IntoIndexes, MutMetadata};

use super::metadata::Metadata;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use std::collections::HashMap;
use url::Url;

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
#[serde(rename_all = "snake_case")]
pub enum WorkloadType {
    #[default]
    HoloChainDht,
    StaticContent,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct WorkloadParameters {
    /// The uploaded happ blob object id
    pub blob_object_id: Option<String>,

    /// network seed
    pub network_seed: Option<String>,

    /// membrane proof
    pub memproof: Option<HashMap<String, String>>,

    /// bootstrap server url
    pub bootstrap_server_url: Option<Url>,

    /// signal server url
    pub signal_server_url: Option<Url>,

    /// stun server urls
    pub stun_server_urls: Option<Vec<Url>>,

    /// holochain feature flags
    pub holochain_feature_flags: Option<Vec<String>>,

    /// holochain version
    pub holochain_version: Option<String>,

    /// HTTP gateway enable flag
    pub http_gw_enable: bool,

    /// HTTP gateway allowed functions
    pub http_gw_allowed_fns: Option<Vec<String>>,
}

pub fn get_default_workload_name() -> String {
    "default_workload".to_string()
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct Workload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,

    /// the user can filter using the workload name
    /// this is a user defined string
    #[serde(default = "get_default_workload_name")]
    pub name: String,

    /// this can be set by the user to fetch the latest workload with a specific tag
    /// e.g. holo-gateway@latest
    /// no versioning is requried and the developer should make their own versioning system or use `_id`
    pub tag: Option<String>,

    /// the execution policy for the workload
    pub execution_policy: ExecutionPolicy,

    /// the type of workload being deployed
    pub workload_type: WorkloadType,

    /// the required parameters for the workload. Some of these won't be required depending on the workload type.
    pub parameters: WorkloadParameters,
}

impl IntoIndexes for Workload {
    fn into_indices(self) -> anyhow::Result<Vec<(Document, Option<IndexOptions>)>> {
        let indices = vec![];
        Ok(indices)
    }
}

impl MutMetadata for Workload {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}