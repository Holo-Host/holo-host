use db_utils::schemas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPolicyVisibilityDto {
    Public,
    Private,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct ExecutionPolicyDto {
    pub jurisdictions: Vec<String>,
    pub regions: Vec<String>,
    pub instances: i32,
    pub visibility: ExecutionPolicyVisibilityDto,
}
pub fn execution_policy_from_dto(
    execution_policy_dto: ExecutionPolicyDto,
) -> schemas::workload::ExecutionPolicy {
    schemas::workload::ExecutionPolicy {
        jurisdictions: execution_policy_dto.jurisdictions,
        instances: execution_policy_dto.instances,
        regions: execution_policy_dto.regions,
        visibility: match execution_policy_dto.visibility {
            ExecutionPolicyVisibilityDto::Public => {
                schemas::workload::ExecutionPolicyVisibility::Public
            }
            ExecutionPolicyVisibilityDto::Private => {
                schemas::workload::ExecutionPolicyVisibility::Private
            }
        },
    }
}
pub fn execution_policy_to_dto(
    execution_policy: schemas::workload::ExecutionPolicy,
) -> ExecutionPolicyDto {
    ExecutionPolicyDto {
        jurisdictions: execution_policy.jurisdictions,
        instances: execution_policy.instances,
        regions: execution_policy.regions,
        visibility: match execution_policy.visibility {
            schemas::workload::ExecutionPolicyVisibility::Public => {
                ExecutionPolicyVisibilityDto::Public
            }
            schemas::workload::ExecutionPolicyVisibility::Private => {
                ExecutionPolicyVisibilityDto::Private
            }
        },
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct CreateWorkloadDto {
    pub execution_policy: ExecutionPolicyDto,
    pub bootstrap_server_url: Option<String>,
    pub signal_server_url: Option<String>,
    pub network_speed: Option<String>,
    pub memproof: Option<HashMap<String, String>>,
    pub http_gw_enable: bool,
    pub http_gw_allowed_fns: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WorkloadDto {
    pub id: String,
    pub manifest_id: String,
    pub execution_policy: ExecutionPolicyDto,
    pub network_seed: Option<String>,
    pub http_gw_enable: bool,
    pub http_gw_allowed_fns: Option<Vec<String>>,
    // pub bootstrap_server_url: Option<String>,
    // pub signal_server_url: Option<String>,
    // pub memproof: Option<HashMap<String, String>>,
}
