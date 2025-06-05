use db_utils::schemas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(CreateWorkloadDto, WorkloadTemplateDto, WorkloadPropertiesDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WorkloadTemplateDto {
    #[schema(value_type = String, example = "template_id")]
    pub blake3_hash: String,

    #[schema(value_type = Vec<String>, example = json!(vec!["https://stun1.example.com".to_string(), "https://stun2.example.com".to_string()]))]
    pub stun_server_urls: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, example = "0.0.1")]
    pub holochain_version: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Vec<String>>, example = json!(vec!["feature1", "feature2"]))]
    pub holochain_feature_flags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WorkloadPropertiesDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, example = "network_seed_value")]
    pub network_seed: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<HashMap<String, String>>, example = json!({"key1": "value1", "key2": "value2"}))]
    pub memproof: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, example = "https://example.com/bootstrap")]
    pub bootstrap_server_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>, example = "https://example.com/signal")]
    pub signal_server_url: Option<String>,

    #[schema(value_type = bool, example = false)]
    pub http_gw_enable: bool,

    #[schema(value_type = Vec<String>, example = json!(vec!["function1".to_string(), "function2".to_string()]))]
    pub http_gw_allowed_fns: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<i32>, example = 1)]
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
