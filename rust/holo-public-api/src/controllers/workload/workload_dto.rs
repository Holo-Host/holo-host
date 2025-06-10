use db_utils::schemas;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use utoipa::{OpenApi, ToSchema};

// Serde helper functions to convert empty values to None during deserialization
mod serde_helpers {
    use super::*;
    
    pub fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<String>::deserialize(deserializer)?;
        Ok(opt.filter(|s| !s.is_empty()))
    }
    
    pub fn empty_vec_as_none<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let opt = Option::<Vec<T>>::deserialize(deserializer)?;
        Ok(opt.filter(|v| !v.is_empty()))
    }
    
    pub fn empty_hashmap_as_none<'de, D>(deserializer: D) -> Result<Option<HashMap<String, String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt = Option::<HashMap<String, String>>::deserialize(deserializer)?;
        Ok(opt.filter(|m| !m.is_empty()))
    }
}

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
    #[serde(deserialize_with = "serde_helpers::empty_string_as_none")]
    pub network_seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_hashmap_as_none")]
    pub memproof: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_string_as_none")]
    pub bootstrap_server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_string_as_none")]
    pub signal_server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_vec_as_none")]
    pub stun_server_urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_vec_as_none")]
    pub holochain_feature_flags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_string_as_none")]
    pub holochain_version: Option<String>,
    pub http_gw_enable: bool,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "serde_helpers::empty_vec_as_none")]
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

/// use serde-based JSON to convert schema to dto
fn convert_via_serde<T, U>(data: T) -> Result<U, serde_json::Error>
where
    T: serde::Serialize,
    U: serde::de::DeserializeOwned,
{
    serde_json::to_string(&data).and_then(|json_str| serde_json::from_str(&json_str))
}

fn to_workload_state_dto(state: schemas::workload::WorkloadState) -> WorkloadStateDto {
    convert_via_serde(state).unwrap_or_else(|e| {
        tracing::error!(
            "Failed to convert workload state to dto. Defaulting to Unknown. Err={:?}",
            e
        );
        WorkloadStateDto::Unknown("conversion_failed".to_string())
    })
}

pub fn to_manifest_dto(data: schemas::workload::WorkloadManifest) -> WorkloadManifestDto {
    convert_via_serde(data).unwrap_or_else(|e| {
        tracing::error!(
            "Failed to convert manifest schema to dto. Defaulting to a value of `None`.  Err={:?}",
            e
        );
        WorkloadManifestDto::None
    })
}

pub fn to_workload_dto(data: schemas::workload::Workload) -> WorkloadDto {
    convert_via_serde(data.clone()).unwrap_or_else(|e| {
        tracing::error!("Failed to convert workload schema to dto.  Falling back to the default dto structure.  Err={:?}", e);

        // Fall back to a default dto structure to avoid unnecessary response failures
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
                id: data.status.id.unwrap_or_default().to_hex(),
                desired: to_workload_state_dto(data.status.desired),
                actual: to_workload_state_dto(data.status.actual),
                payload: None,
            },
            manifest: to_manifest_dto(data.manifest),
        }
    })
}
