use db_utils::schemas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(CreateWorkloadDto, WorkloadTemplateDto, WorkloadPropertiesDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WorkloadTemplateDto {
    pub blake3_hash: String,
    pub stun_server_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holochain_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holochain_feature_flags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WorkloadPropertiesDto {
    pub network_seed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memproof: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_server_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_server_url: Option<String>,
    pub http_gw_enable: bool,
    pub http_gw_allowed_fns: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instances: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct CreateWorkloadDto {
    pub template: WorkloadTemplateDto,
    pub properties: WorkloadPropertiesDto,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WorkloadDto {
    pub id: String,
    pub properties: WorkloadPropertiesDto,
    pub status: schemas::workload::WorkloadState,
}
