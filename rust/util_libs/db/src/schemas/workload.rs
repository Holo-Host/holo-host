use anyhow::Result;
use bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::options::IndexOptions;
use semver::{BuildMetadata, Prerelease};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use strum::{EnumDiscriminants, EnumString, FromRepr};
use strum_macros::AsRefStr;
use url::Url;
use utoipa::ToSchema;

use super::alias::SemVer;
use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Collection name for workload documents
pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

/// Enumeration of possible workload states
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumDiscriminants, FromRepr, ToSchema,
)]
#[strum_discriminants(
    derive(EnumString, Serialize, Deserialize),
    repr(usize),
    strum(serialize_all = "snake_case")
)]
pub enum WorkloadState {
    /// Workload reported by developer
    Reported,
    /// Workload assigned to host
    Assigned,
    /// Workload installation pending on host device
    Pending,
    // /// Workload installed on host device
    // Installed,
    /// Workload running on host device
    Running,
    /// Workload update completed
    Updated,
    /// Workload marked for deletion
    Deleted,
    /// Workload uninstalled from host device
    Uninstalled,
    /// Error state with message
    Error(String),
    /// Unknown state with context
    Unknown(String),
}

/// Status information for a workload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadStatus {
    /// Optional MongoDB ObjectId for the status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// Desired state of the workload
    pub desired: WorkloadState,
    /// Actual current state of the workload
    pub actual: WorkloadState,
    pub payload: WorkloadStatePayload,
}

/// Resource capacity requirements for a workload
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Capacity {
    /// Required drive space in GiB
    pub drive: i64,
    /// Required CPU cores
    pub cores: i64,
}

/// System specifications for a workload
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemSpecs {
    /// Resource capacity requirements
    pub capacity: Capacity,
    /// Required network speed in Mbps
    pub avg_network_speed: i64,
    /// Required uptime as a decimal between 0-1
    pub avg_uptime: f64,
}

impl PartialEq for SystemSpecs {
    fn eq(&self, other: &Self) -> bool {
        self.capacity == other.capacity
            && self.avg_network_speed == other.avg_network_speed
            && (self.avg_uptime - other.avg_uptime).abs() < 1e-9
    }
}

/// Workload document schema representing a deployable application
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    /// MongoDB ObjectId of the workload document
    pub _id: ObjectId,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the user who created this workload
    pub assigned_developer: ObjectId,
    /// Semantic version of the workload
    pub version: SemVer,
    /// Minimum number of hosts required
    pub min_hosts: i32,
    /// System requirements for the workload
    pub system_specs: SystemSpecs,
    /// List of hosts this workload is assigned to
    pub assigned_hosts: Vec<ObjectId>,
    /// Current status of the workload
    pub status: WorkloadStatus,
    pub manifest: WorkloadManifest, // (Includes information about everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum WorkloadManifest {
    None,
    ExtraContainerPath { extra_container_path: String },
    ExtraContainerStorePath { store_path: PathBuf },
    ExtraContainerBuildCmd { nix_args: Box<[String]> },
    HolochainDhtV1(Box<WorkloadManifestHolochainDhtV1>),
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
pub enum WorkloadStatePayload {
    #[default]
    None,
    HolochainDhtV1(Bson),
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct WorkloadManifestHolochainDhtV1 {
    #[cfg_attr(feature = "clap", arg(long, value_parser = parse_happ_binary))]
    pub happ_binary: HappBinaryFormat,

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

impl<'de> Deserialize<'de> for WorkloadManifestHolochainDhtV1 {
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
                map.remove($field)
                    .map(|v| serde_json::from_value::<$ty>(v).map_err(de::Error::custom))
                    .transpose()?
            };
        }

        Ok(WorkloadManifestHolochainDhtV1 {
            happ_binary,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum HappBinaryFormat {
    HappBinaryUrl(Url),
    HappBinaryBlake3Hash(String),
}

impl std::fmt::Display for HappBinaryFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HappBinaryFormat::HappBinaryUrl(url) => write!(f, "URL: {}", url),
            HappBinaryFormat::HappBinaryBlake3Hash(hash) => write!(f, "Blake3Hash: {}", hash),
        }
    }
}

<<<<<<< HEAD
=======
/// Parse into the `HappBinaryFormat` from the clap cli arg (str)
>>>>>>> main
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

<<<<<<< HEAD
=======
#[derive(Serialize, Clone, Debug)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct WorkloadManifestHolochainDhtV1 {
    #[cfg_attr(feature = "clap", arg(long, value_parser = parse_happ_binary))]
    pub happ_binary: HappBinaryFormat,

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

impl<'de> Deserialize<'de> for WorkloadManifestHolochainDhtV1 {
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
                map.remove($field)
                    .map(|v| serde_json::from_value::<$ty>(v).map_err(de::Error::custom))
                    .transpose()?
            };
        }

        Ok(WorkloadManifestHolochainDhtV1 {
            happ_binary,
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

>>>>>>> main
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

impl Default for Workload {
    /// Creates a default workload configuration with:
    /// - Version 0.0.0
    /// - Minimum 1 host
    /// - 1 GiB drive space
    /// - 1 cores
    /// - 0 Mbps network speed
    /// - 0% uptime requirement
    fn default() -> Self {
        let version = semver::Version {
            major: 0,
            minor: 0,
            patch: 0,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        };

        let semver = version.to_string();

        Self {
            _id: ObjectId::new(),
            metadata: Metadata {
                is_deleted: false,
                created_at: Some(DateTime::now()),
                updated_at: Some(DateTime::now()),
                deleted_at: None,
            },
            version: semver,
            assigned_developer: ObjectId::new(),
            min_hosts: 1,
            system_specs: SystemSpecs {
                capacity: Capacity { drive: 1, cores: 1 },
                avg_network_speed: 0,
                avg_uptime: 0f64,
            },
            assigned_hosts: Vec::new(),
            status: WorkloadStatus {
                id: None,
                desired: WorkloadState::Unknown("default state".to_string()),
                actual: WorkloadState::Unknown("default state".to_string()),
                payload: WorkloadStatePayload::None,
            },
            manifest: WorkloadManifest::None,
        }
    }
}

impl IntoIndexes for Workload {
    /// Defines MongoDB indices for the Workload collection
    ///
    /// Creates an index for:
    /// - assigned_developer
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Owner Index
        let assigned_developer_index_doc = doc! { "assigned_developer": 1 };
        let assigned_developer_index_opts = Some(
            IndexOptions::builder()
                .name(Some("assigned_developer_index".to_string()))
                .build(),
        );
        indices.push((assigned_developer_index_doc, assigned_developer_index_opts));

        Ok(indices)
    }
}

impl MutMetadata for Workload {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
