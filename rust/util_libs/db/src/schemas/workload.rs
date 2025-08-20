use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};
use anyhow::Result;
use bson::oid::ObjectId;
use bson::{doc, Document};
use mongodb::options::IndexOptions;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicyVisibility {
    #[default]
    Public,
    Private,
}

fn default_instances() -> i32 {
    1
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ExecutionPolicy {
    /// the jurisdictions to deploy the workload in
    /// this maps to the jurisdiction code in the jurisdiction collection
    pub jurisdictions: Vec<String>,
    /// the region to deploy the workload in
    /// this maps to the region code in the region collection
    pub regions: Vec<String>,
    /// minimum number of instances required for this workload (NB: previously called min_hosts)
    #[serde(default = "default_instances")]
    pub instances: i32,
    /// the visibility of the workload on hosts
    pub visibility: ExecutionPolicyVisibility,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct Context {
    #[cfg_attr(feature = "clap", arg(long))]
    pub http_gw_enable: bool,

    #[cfg_attr(feature = "clap", arg(long))]
    pub http_gw_allowed_fns: Option<Vec<String>>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub network_seed: Option<String>,
}

impl<'de> Deserialize<'de> for Context {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut map: HashMap<String, JsonValue> = Deserialize::deserialize(deserializer)?;

        macro_rules! pop_field {
            ($field:literal, $ty:ty) => {
                map.remove($field).and_then(|v| {
                    if v.is_null() {
                        None
                    } else {
                        serde_json::from_value::<$ty>(v).ok()
                    }
                })
            };
        }

        Ok(Context {
            network_seed: pop_field!("network_seed", String),
            http_gw_enable: pop_field!("http_gw_enable", bool).unwrap_or(false),
            http_gw_allowed_fns: pop_field!("http_gw_allowed_fns", Vec<String>),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    pub metadata: Metadata,

    /// MongoDB ObjectId of the user document
    pub _id: ObjectId,

    /// The User that owns this resource (NB: previously called the assigned_developer)
    pub owner: ObjectId,

    /// The manifest for this workload
    pub manifest_id: ObjectId,

    pub execution_policy: ExecutionPolicy,

    pub context: Context, // NB: previously the `WorkloadManifestHolochainDhtV1` - a variant that was a part of the `WorkloadManifest`
}

impl Workload {
    pub fn new(owner: ObjectId, manifest_id: ObjectId) -> Self {
        Workload {
            _id: ObjectId::new(),
            metadata: Metadata::default(),
            owner,
            manifest_id,
            execution_policy: ExecutionPolicy::default(),
            context: Context::default(),
        }
    }
}

impl Default for Workload {
    fn default() -> Self {
        let placeholder_owner = ObjectId::new();
        let placeholder_manifest = ObjectId::new();
        Workload::new(placeholder_owner, placeholder_manifest)
    }
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
