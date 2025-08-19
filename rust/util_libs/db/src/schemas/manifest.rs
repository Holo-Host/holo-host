use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;

pub const MANIFEST_COLLECTION_NAME: &str = "manifest";

// todo: add parameters to enum
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadType {
    HoloChainDht(Box<HoloChainDhtParameters>),
    StaticContent(Box<StaticContentParameters>),
    WebBridge(Box<WebBridgeParameters>),
}

impl Default for WorkloadType {
    fn default() -> Self {
        WorkloadType::HoloChainDht(Default::default())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct HoloChainDhtParameters {
    /// The uploaded happ blob object id
    pub blob_object_id: Option<String>,

    /// stun server urls
    pub stun_server_urls: Option<Vec<String>>,

    /// holochain feature flags
    pub holochain_feature_flags: Option<Vec<String>>,

    /// holochain version
    pub holochain_version: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct StaticContentParameters {}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct WebBridgeParameters {}

pub fn get_default_workload_name() -> String {
    "default_workload".to_string()
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct Manifest {
    pub _id: ObjectId,
    pub metadata: Metadata,
    pub owner: ObjectId,

    /// the user can filter using the workload name
    /// this is a user defined string
    #[serde(default = "get_default_workload_name")]
    pub name: String,

    /// this can be set by the user to fetch the latest workload with a specific tag
    /// e.g. holo-gateway@latest
    /// no versioning is requried and the developer should make their own versioning system or use `_id`
    pub tag: Option<String>,

    /// the type of workload being deployed
    pub workload_type: WorkloadType,
}

impl IntoIndexes for Manifest {
    fn into_indices(self) -> anyhow::Result<Vec<(Document, Option<IndexOptions>)>> {
        let indices = vec![];
        Ok(indices)
    }
}

impl MutMetadata for Manifest {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
