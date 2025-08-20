use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};
use anyhow::Result;
use bson::oid::ObjectId;
use bson::{doc, Document};
use mongodb::options::IndexOptions;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use url::Url;

pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicyVisibility {
    #[default]
    Public,
    Private,
}

fn default_instances() -> i32 {
    1
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
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

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct Workload {
    /// MongoDB ObjectId of the user document
    pub _id: ObjectId,

    pub metadata: Metadata,

    /// the user that owns this resource (NB: previously called the assigned_developer)
    pub owner: ObjectId,

    pub execution_policy: ExecutionPolicy,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub network_seed: Option<String>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ',', value_parser = parse_key_val::<String, String>))]
    pub memproof: Option<HashMap<String, String>>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub bootstrap_server_url: Option<Url>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub signal_server_url: Option<Url>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub stun_server_urls: Option<Vec<Url>>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub holochain_feature_flags: Option<Vec<String>>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub holochain_version: Option<String>,

    #[cfg_attr(feature = "clap", arg(long))]
    pub http_gw_enable: bool,

    #[cfg_attr(feature = "clap", arg(long))]
    pub http_gw_allowed_fns: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for Workload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut map: HashMap<String, JsonValue> = Deserialize::deserialize(deserializer)?;

        // let happ_binary = if let Some(hb) = map.remove("happ_binary") {
        //     serde_json::from_value(hb).map_err(de::Error::custom)?
        // } else if let Some(url) = map.remove("happ_binary_url") {
        //     let url: Url = serde_json::from_value(url).map_err(de::Error::custom)?;
        //     HappBinaryFormat::HappBinaryUrl(url)
        // } else if let Some(hash) = map.remove("happ_binary_hash") {
        //     let hash: String = serde_json::from_value(hash).map_err(de::Error::custom)?;
        //     HappBinaryFormat::HappBinaryBlake3Hash(hash)
        // } else {
        //     return Err(de::Error::missing_field(
        //         "happ_binary, happ_binary_url, or happ_binary_hash",
        //     ));
        // };

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

        Ok(Workload {
            _id: ObjectId::new(),
            metadata: Metadata::default(),
            owner: ObjectId::new(),
            execution_policy: ExecutionPolicy::default(),
            network_seed: pop_field!("network_seed", String),
            memproof: pop_field!("memproof", HashMap<String, String>),
            bootstrap_server_url: pop_field!("bootstrap_server_url", Url),
            signal_server_url: pop_field!("signal_server_url", Url),
            stun_server_urls: pop_field!("stun_server_urls", Vec<Url>),
            holochain_feature_flags: pop_field!("holochain_feature_flags", Vec<String>),
            holochain_version: pop_field!("holochain_version", String),
            http_gw_enable: pop_field!("http_gw_enable", bool).unwrap_or(false),
            http_gw_allowed_fns: pop_field!("http_gw_allowed_fns", Vec<String>),
        })
    }
}

/// Parse a single key-value pair
fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
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
