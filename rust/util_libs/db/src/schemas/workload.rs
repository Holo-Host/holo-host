use bson::{doc, oid::ObjectId, Bson, DateTime, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use semver::{BuildMetadata, Prerelease};
use std::collections::HashMap;
use std::path::PathBuf;
use strum::{EnumDiscriminants, EnumString, FromRepr};
use strum_macros::AsRefStr;
use url::Url;

use crate::mongodb::{MutMetadata, IntoIndexes};
use super::metadata::Metadata;
use super::alias::SemVer;

/// Collection name for workload documents
pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

/// Enumeration of possible workload states
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumDiscriminants, FromRepr,
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
    /// Workload installed on host device
    Installed,
    /// Workload running on host device
    Running,
    /// Workload is being updated
    Updating,
    /// Workload update completed
    Updated,
    /// Workload marked for deletion
    Deleted,
    /// Workload links removed
    Removed,
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
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
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

/// Workload document schema representing a deployable application
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    /// MongoDB ObjectId of the workload document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the developer who created this workload
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug, clap::Args)]
pub struct WorkloadManifestHolochainDhtV1 {
    #[arg(long, value_delimiter = ',')]
    pub happ_binary_url: Url,
    #[arg(long, value_delimiter = ',')]
    pub network_seed: String,
    #[arg(long, value_delimiter = ',', value_parser = parse_key_val::<String, String>)]
    pub memproof: Option<HashMap<String, String>>,
    #[arg(long, value_delimiter = ',')]
    pub bootstrap_server_url: Option<Url>,
    #[arg(long, value_delimiter = ',')]
    pub signal_server_url: Option<Url>,
    #[arg(long, value_delimiter = ',')]
    pub stun_server_urls: Option<Vec<Url>>,
    #[arg(long, value_delimiter = ',')]
    pub holochain_feature_flags: Option<Vec<String>>,
    #[arg(long, value_delimiter = ',')]
    pub holochain_version: Option<String>,
    #[arg(long)]
    pub http_gw_enable: bool,
    #[arg(long)]
    pub http_gw_allowed_fns: Option<Vec<String>>,
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
            _id: None,
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

        //  Add Assigned Developer Index
        let developer_index_doc = doc! { "assigned_developer": 1 };
        let developer_index_opts = Some(
            IndexOptions::builder()
                .name(Some("assigned_developer_index".to_string()))
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        Ok(indices)
    }
}

impl MutMetadata for Workload {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}