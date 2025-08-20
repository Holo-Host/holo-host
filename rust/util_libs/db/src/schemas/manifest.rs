use crate::mongodb::traits::{IntoIndexes, MutMetadata};
use crate::schemas::{metadata::Metadata, parse_key_val};
use anyhow::Result;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use url::Url;

pub const MANIFEST_COLLECTION_NAME: &str = "manifest";

pub fn get_default_manifest_name() -> String {
    "default_manifest".to_string()
}

// todo: add parameters to enum
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ManifestType {
    HolochainDht(Box<HolochainDhtParameters>),
    StaticContent(Box<StaticContentParameters>),
    WebBridge(Box<WebBridgeParameters>),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct WebBridgeParameters {}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct StaticContentParameters {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum HappBinaryFormat {
    /// The uploaded happ blob object id
    HappBinaryBlake3Hash(String),
    HappBinaryUrl(Url),
}

impl std::fmt::Display for HappBinaryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HappBinaryFormat::HappBinaryUrl(url) => write!(f, "URL: {}", url),
            HappBinaryFormat::HappBinaryBlake3Hash(hash) => write!(f, "Blake3Hash: {}", hash),
        }
    }
}

/// Parse into the `HappBinaryFormat` from the clap cli arg (str)
#[cfg(feature = "clap")]
fn parse_happ_binary(
    s: &str,
) -> Result<HappBinaryFormat, Box<dyn std::error::Error + Send + Sync + 'static>> {
    if s.starts_with("http://") || s.starts_with("https://") {
        let url = Url::parse(s)?;
        Ok(HappBinaryFormat::HappBinaryUrl(url))
    } else {
        // assume (for now) that it's a blake3 hash if it's not a valid Url
        Ok(HappBinaryFormat::HappBinaryBlake3Hash(s.to_string()))
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct HolochainDhtParameters {
    // pub blob_object_id: Option<String>,
    #[cfg_attr(feature = "clap", arg(long, value_parser = parse_happ_binary))]
    pub happ_binary: HappBinaryFormat,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub holochain_version: Option<String>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub holochain_feature_flags: Option<Vec<String>>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub stun_server_urls: Option<Vec<Url>>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub signal_server_url: Option<Url>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ','))]
    pub bootstrap_server_url: Option<Url>,

    #[cfg_attr(feature = "clap", arg(long, value_delimiter = ',', value_parser = parse_key_val::<String, String>))]
    pub memproofs: Option<HashMap<String, String>>,
}

impl<'de> Deserialize<'de> for HolochainDhtParameters {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut map: HashMap<String, JsonValue> = Deserialize::deserialize(deserializer)?;

        let happ_binary = if let Some(hb) = map.remove("happ_binary") {
            serde_json::from_value(hb).map_err(de::Error::custom)?
        } else if let Some(url) = map.remove("happ_binary_url") {
            let url: Url = serde_json::from_value(url).map_err(de::Error::custom)?;
            HappBinaryFormat::HappBinaryUrl(url)
        } else if let Some(hash) = map.remove("happ_binary_hash") {
            let hash: String = serde_json::from_value(hash).map_err(de::Error::custom)?;
            HappBinaryFormat::HappBinaryBlake3Hash(hash)
        } else {
            return Err(de::Error::missing_field(
                "happ_binary, happ_binary_url, or happ_binary_hash",
            ));
        };

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

        Ok(HolochainDhtParameters {
            happ_binary,
            holochain_version: pop_field!("holochain_version", String),
            holochain_feature_flags: pop_field!("holochain_feature_flags", Vec<String>),
            stun_server_urls: pop_field!("stun_server_urls", Vec<Url>),
            signal_server_url: pop_field!("signal_server_url", Url),
            bootstrap_server_url: pop_field!("bootstrap_server_url", Url),
            memproofs: pop_field!("memproofs", HashMap<String, String>),
        })
    }
}

impl HolochainDhtParameters {
    pub fn with_default_params(happ_binary: HappBinaryFormat) -> Self {
        HolochainDhtParameters {
            happ_binary,
            holochain_version: None,
            holochain_feature_flags: None,
            stun_server_urls: None,
            signal_server_url: None,
            bootstrap_server_url: None,
            memproofs: None,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Manifest {
    pub _id: ObjectId,
    pub metadata: Metadata,
    pub owner: ObjectId,

    /// the user can filter using the workload name
    /// this is a user defined string
    #[serde(default = "get_default_manifest_name")]
    pub name: String,

    /// this can be set by the user to fetch the latest workload with a specific tag
    /// e.g. holo-gateway@latest
    /// no versioning is requried and the developer should make their own versioning system or use `_id`
    pub tag: Option<String>,

    pub manifest_type: ManifestType,
}

impl Manifest {
    pub fn new(
        owner: ObjectId,
        name: Option<String>,
        tag: Option<String>,
        manifest_type: ManifestType,
    ) -> Self {
        Manifest {
            _id: ObjectId::new(),
            metadata: Metadata::default(),
            owner,
            name: name.unwrap_or_default(),
            tag,
            manifest_type,
        }
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Manifest {
            _id: ObjectId::new(),
            metadata: Metadata::default(),
            owner: ObjectId::new(),
            name: get_default_manifest_name(),
            tag: None,
            manifest_type: ManifestType::HolochainDht(Box::new(
                HolochainDhtParameters::with_default_params(
                    HappBinaryFormat::HappBinaryBlake3Hash("placeholder_happ_hash".to_string()),
                ),
            )),
        }
    }
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
