use db_utils::schemas;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct HoloChainDhtParametersDto {
    pub blob_object_id: Option<String>,
    pub stun_server_urls: Option<Vec<String>>,
    pub holochain_feature_flags: Option<Vec<String>>,
    pub holochain_version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct StaticContentParametersDto {}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WebBridgeParametersDto {}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub enum WorkloadTypeDto {
    HoloChainDht(Box<HoloChainDhtParametersDto>),
    StaticContent(Box<StaticContentParametersDto>),
    WebBridge(Box<WebBridgeParametersDto>),
}
pub fn workload_type_from_dto(
    workload_type_dto: WorkloadTypeDto,
) -> schemas::manifest::WorkloadType {
    match workload_type_dto {
        WorkloadTypeDto::HoloChainDht(parameters) => schemas::manifest::WorkloadType::HoloChainDht(
            Box::new(schemas::manifest::HoloChainDhtParameters {
                blob_object_id: parameters.blob_object_id,
                holochain_feature_flags: parameters.holochain_feature_flags,
                holochain_version: parameters.holochain_version,
                stun_server_urls: parameters.stun_server_urls,
            }),
        ),
        WorkloadTypeDto::StaticContent(_) => schemas::manifest::WorkloadType::StaticContent(
            Box::new(schemas::manifest::StaticContentParameters {}),
        ),
        WorkloadTypeDto::WebBridge(_) => schemas::manifest::WorkloadType::WebBridge(Box::new(
            schemas::manifest::WebBridgeParameters {},
        )),
    }
}
pub fn workload_type_to_dto(workload_type: schemas::manifest::WorkloadType) -> WorkloadTypeDto {
    match workload_type {
        schemas::manifest::WorkloadType::HoloChainDht(parameters) => {
            WorkloadTypeDto::HoloChainDht(Box::new(HoloChainDhtParametersDto {
                blob_object_id: parameters.blob_object_id,
                holochain_feature_flags: parameters.holochain_feature_flags,
                holochain_version: parameters.holochain_version,
                stun_server_urls: parameters.stun_server_urls,
            }))
        }
        schemas::manifest::WorkloadType::StaticContent(_) => {
            WorkloadTypeDto::StaticContent(Box::new(StaticContentParametersDto {}))
        }
        schemas::manifest::WorkloadType::WebBridge(_) => {
            WorkloadTypeDto::WebBridge(Box::new(WebBridgeParametersDto {}))
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct CreateManifestDto {
    pub name: String,
    pub tag: Option<String>,
    pub workload_type: WorkloadTypeDto,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct ManifestDto {
    pub id: String,
    pub name: String,
    pub tag: Option<String>,
    pub workload_type: WorkloadTypeDto,
}
