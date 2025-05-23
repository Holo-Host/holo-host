use db_utils::schemas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(components(schemas(WorkloadDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadStateDto {
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

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct CapacityDto {
    pub drive: i64,
    pub cores: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SystemSpecsDto {
    pub capacity: CapacityDto,
    pub avg_network_speed: i64,
    pub avg_uptime: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct WorkloadStatusDto {
    pub id: String,
    pub desired: WorkloadStateDto,
    pub actual: WorkloadStateDto,
    pub payload: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct WorkloadManifestHolochainDhtV1Dto {
    pub happ_binary_url: String,
    pub network_seed: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memproof: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stun_server_urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holochain_feature_flags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holochain_version: Option<String>,
    pub http_gw_enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_gw_allowed_fns: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadManifestDto {
    None,
    ExtraContainerPath { extra_container_path: String },
    ExtraContainerStorePath { store_path: String },
    ExtraContainerBuildCmd { nix_args: Box<[String]> },
    HolochainDhtV1(Box<WorkloadManifestHolochainDhtV1Dto>),
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct WorkloadDto {
    /// unique identifier for the workload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Reference to the user who created this workload
    pub assigned_developer: String,
    /// Semantic version of the workload
    pub version: String,
    /// Minimum number of hosts required
    pub min_hosts: i32,
    /// System requirements for the workload
    pub system_specs: SystemSpecsDto,
    /// List of hosts this workload is assigned to
    pub assigned_hosts: Vec<String>,
    /// Current status of the workload
    pub status: WorkloadStatusDto,
    pub manifest: WorkloadManifestDto,
}

fn to_workload_state_dto(state: schemas::workload::WorkloadState) -> WorkloadStateDto {
    match state {
        schemas::workload::WorkloadState::Reported => WorkloadStateDto::Reported,
        schemas::workload::WorkloadState::Assigned => WorkloadStateDto::Assigned,
        schemas::workload::WorkloadState::Pending => WorkloadStateDto::Pending,
        schemas::workload::WorkloadState::Installed => WorkloadStateDto::Installed,
        schemas::workload::WorkloadState::Running => WorkloadStateDto::Running,
        schemas::workload::WorkloadState::Updating => WorkloadStateDto::Updating,
        schemas::workload::WorkloadState::Updated => WorkloadStateDto::Updated,
        schemas::workload::WorkloadState::Deleted => WorkloadStateDto::Deleted,
        schemas::workload::WorkloadState::Removed => WorkloadStateDto::Removed,
        schemas::workload::WorkloadState::Uninstalled => WorkloadStateDto::Uninstalled,
        schemas::workload::WorkloadState::Error(msg) => WorkloadStateDto::Error(msg),
        schemas::workload::WorkloadState::Unknown(ctx) => WorkloadStateDto::Unknown(ctx),
    }
}

pub fn to_manifest_dto(data: schemas::workload::WorkloadManifest) -> WorkloadManifestDto {
    match data {
        schemas::workload::WorkloadManifest::None => WorkloadManifestDto::None,
        schemas::workload::WorkloadManifest::ExtraContainerPath {
            extra_container_path,
        } => WorkloadManifestDto::ExtraContainerPath {
            extra_container_path,
        },
        schemas::workload::WorkloadManifest::ExtraContainerStorePath { store_path } => {
            WorkloadManifestDto::ExtraContainerStorePath {
                store_path: store_path
                    .to_str()
                    .expect("failed to convert store_path to string")
                    .to_string(),
            }
        }
        schemas::workload::WorkloadManifest::ExtraContainerBuildCmd { nix_args } => {
            WorkloadManifestDto::ExtraContainerBuildCmd { nix_args }
        }
        schemas::workload::WorkloadManifest::HolochainDhtV1(data) => {
            WorkloadManifestDto::HolochainDhtV1(Box::new(WorkloadManifestHolochainDhtV1Dto {
                happ_binary_url: data.happ_binary_url.to_string(),
                network_seed: data.network_seed,
                memproof: data.memproof,
                bootstrap_server_url: data.bootstrap_server_url.map(|url| url.to_string()),
                signal_server_url: data.signal_server_url.map(|url| url.to_string()),
                stun_server_urls: data
                    .stun_server_urls
                    .map(|data| data.into_iter().map(|url| url.to_string()).collect()),
                holochain_feature_flags: data.holochain_feature_flags,
                holochain_version: data.holochain_version,
                http_gw_enable: data.http_gw_enable,
                http_gw_allowed_fns: data.http_gw_allowed_fns,
            }))
        }
    }
}

pub fn to_workload_dto(data: schemas::workload::Workload) -> WorkloadDto {
    WorkloadDto {
        id: data._id.map(|id| id.to_hex()),
        assigned_developer: data.assigned_developer.to_hex(),
        version: data.version.to_string(),
        min_hosts: data.min_hosts,
        system_specs: SystemSpecsDto {
            avg_uptime: data.system_specs.avg_uptime,
            avg_network_speed: data.system_specs.avg_network_speed,
            capacity: CapacityDto {
                drive: data.system_specs.capacity.drive,
                cores: data.system_specs.capacity.cores,
            },
        },
        assigned_hosts: data
            .assigned_hosts
            .iter()
            .map(|host| host.to_hex())
            .collect(),
        status: WorkloadStatusDto {
            id: data.status.id.unwrap().to_hex(),
            desired: to_workload_state_dto(data.status.desired),
            actual: to_workload_state_dto(data.status.actual),
            payload: None,
        },
        manifest: to_manifest_dto(data.manifest),
    }
}

pub fn from_manifest_dto(data: WorkloadManifestDto) -> schemas::workload::WorkloadManifest {
    match data {
        WorkloadManifestDto::None => schemas::workload::WorkloadManifest::None,
        WorkloadManifestDto::ExtraContainerPath {
            extra_container_path,
        } => schemas::workload::WorkloadManifest::ExtraContainerPath {
            extra_container_path: extra_container_path.to_string(),
        },
        WorkloadManifestDto::ExtraContainerStorePath { store_path } => {
            schemas::workload::WorkloadManifest::ExtraContainerStorePath {
                store_path: std::path::PathBuf::from(store_path),
            }
        }
        WorkloadManifestDto::ExtraContainerBuildCmd { nix_args } => {
            schemas::workload::WorkloadManifest::ExtraContainerBuildCmd {
                nix_args: nix_args.clone(),
            }
        }
        WorkloadManifestDto::HolochainDhtV1(data) => {
            schemas::workload::WorkloadManifest::HolochainDhtV1(Box::new(
                schemas::workload::WorkloadManifestHolochainDhtV1 {
                    happ_binary_url: Url::parse(&data.happ_binary_url)
                        .expect("failed to parse url"),
                    network_seed: data.network_seed.to_string(),
                    memproof: data.memproof.clone(),
                    bootstrap_server_url: data
                        .bootstrap_server_url
                        .clone()
                        .map(|url| Url::parse(&url).expect("failed to parse url")),
                    signal_server_url: data
                        .signal_server_url
                        .clone()
                        .map(|url| Url::parse(&url).expect("failed to parse url")),
                    stun_server_urls: data.stun_server_urls.map(|data| {
                        data.into_iter()
                            .map(|url| Url::parse(&url).expect("failed to parse url"))
                            .collect()
                    }),
                    holochain_feature_flags: data.holochain_feature_flags.clone(),
                    holochain_version: data.holochain_version.clone(),
                    http_gw_enable: data.http_gw_enable,
                    http_gw_allowed_fns: data.http_gw_allowed_fns.clone(),
                },
            ))
        }
    }
}
