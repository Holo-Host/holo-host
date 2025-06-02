use bson::oid::ObjectId;
use db_utils::schemas::{
    metadata::Metadata,
    workload_layout::{ExecutionPolicyVisibility, WorkloadLayout, WorkloadType},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(components(schemas(WorkloadDto)))]
pub struct OpenApiSpec;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct ExecutionPolicyDto {
    /// The jurisdictions to deploy the workload in
    /// This maps to the jurisdiction code in the jurisdiction collection
    pub jurisdictions: Vec<String>,
    /// The region to deploy the workload in
    /// This maps to the region code in the region collection
    pub regions: Vec<String>,
    /// Minimum number of instances required for this workload
    pub instances: i32,
    /// The visibility of the workload on hosts
    pub visibility: ExecutionPolicyVisibility,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct WorkloadParametersDto {
    /// The uploaded happ blob object id
    pub blob_object_id: Option<String>,
    /// network seed
    pub network_seed: Option<String>,
    /// membrane proof
    pub memproof: Option<HashMap<String, String>>,
    /// bootstrap server url
    pub bootstrap_server_url: Option<String>,
    /// signal server url
    pub signal_server_url: Option<String>,
    /// stun server urls
    pub stun_server_urls: Option<Vec<String>>,
    /// holochain feature flags
    pub holochain_feature_flags: Option<Vec<String>>,
    /// holochain version
    pub holochain_version: Option<String>,
    /// HTTP gateway enable flag
    pub http_gw_enable: bool,
    /// HTTP gateway allowed functions
    pub http_gw_allowed_fns: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct CreateWorkloadDto {
    pub name: String,
    pub tag: Option<String>,
    pub execution_policy: ExecutionPolicyDto,
    pub workload_type: WorkloadType,
    pub parameters: WorkloadParametersDto,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct WorkloadDto {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub tag: Option<String>,
    pub execution_policy: ExecutionPolicyDto,
    pub workload_type: WorkloadType,
    pub parameters: WorkloadParametersDto,
}

pub fn from_execution_policy_dto(
    dto: ExecutionPolicyDto,
) -> db_utils::schemas::workload_layout::ExecutionPolicy {
    db_utils::schemas::workload_layout::ExecutionPolicy {
        jurisdictions: dto.jurisdictions,
        regions: dto.regions,
        instances: dto.instances,
        visibility: dto.visibility,
    }
}

pub fn from_parameters_dto(
    dto: WorkloadParametersDto,
) -> db_utils::schemas::workload_layout::WorkloadParameters {
    let stun_server_urls = match dto.stun_server_urls {
        Some(dto_stun_server_urls) => {
            let mut stun_server_urls_vec: Vec<url::Url> = vec![];
            for value in dto_stun_server_urls {
                let url = url::Url::parse(&value).expect("Invalid STUN server URL");
                stun_server_urls_vec.push(url);
            }
            Some(stun_server_urls_vec)
        }
        None => None,
    };

    db_utils::schemas::workload_layout::WorkloadParameters {
        blob_object_id: dto.blob_object_id,
        network_seed: dto.network_seed,
        memproof: dto.memproof,
        bootstrap_server_url: dto
            .bootstrap_server_url
            .map(|url| url::Url::parse(&url).expect("Invalid URL")),
        signal_server_url: dto
            .signal_server_url
            .map(|url| url::Url::parse(&url).expect("Invalid URL")),
        stun_server_urls,
        holochain_feature_flags: dto.holochain_feature_flags,
        holochain_version: dto.holochain_version,
        http_gw_enable: dto.http_gw_enable,
        http_gw_allowed_fns: dto.http_gw_allowed_fns,
    }
}

pub fn from_create_workload_dto(dto: CreateWorkloadDto, owner: ObjectId) -> WorkloadLayout {
    WorkloadLayout {
        _id: None,
        owner,
        metadata: Metadata::default(),
        name: dto.name,
        tag: dto.tag,
        execution_policy: from_execution_policy_dto(dto.execution_policy),
        parameters: from_parameters_dto(dto.parameters),
        workload_type: dto.workload_type,
    }
}

pub fn to_execution_policy_dto(
    execution_policy: db_utils::schemas::workload_layout::ExecutionPolicy,
) -> ExecutionPolicyDto {
    ExecutionPolicyDto {
        jurisdictions: execution_policy.jurisdictions,
        regions: execution_policy.regions,
        instances: execution_policy.instances,
        visibility: execution_policy.visibility,
    }
}

pub fn to_parameters_dto(
    parameters: db_utils::schemas::workload_layout::WorkloadParameters,
) -> WorkloadParametersDto {
    WorkloadParametersDto {
        blob_object_id: parameters.blob_object_id,
        network_seed: parameters.network_seed,
        memproof: parameters.memproof,
        bootstrap_server_url: parameters.bootstrap_server_url.map(|url| url.to_string()),
        signal_server_url: parameters.signal_server_url.map(|url| url.to_string()),
        stun_server_urls: parameters
            .stun_server_urls
            .map(|urls| urls.iter().map(|url| url.to_string()).collect()),
        holochain_feature_flags: parameters.holochain_feature_flags,
        holochain_version: parameters.holochain_version,
        http_gw_enable: parameters.http_gw_enable,
        http_gw_allowed_fns: parameters.http_gw_allowed_fns,
    }
}

pub fn to_workload_dto(workload: WorkloadLayout) -> WorkloadDto {
    WorkloadDto {
        id: workload._id.map(|id| id.to_string()).unwrap_or_default(),
        owner: workload.owner.to_string(),
        name: workload.name,
        tag: workload.tag,
        execution_policy: to_execution_policy_dto(workload.execution_policy),
        workload_type: workload.workload_type,
        parameters: to_parameters_dto(workload.parameters),
    }
}
