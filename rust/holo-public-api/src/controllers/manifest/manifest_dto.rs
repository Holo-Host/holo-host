use anyhow::{anyhow, Result};
use db_utils::schemas;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct HolochainDhtParametersDto {
    pub blob_object_id: String,
    pub holochain_version: Option<String>,
    pub holochain_feature_flags: Option<Vec<String>>,
    pub stun_server_urls: Option<Vec<String>>,
    pub signal_server_url: Option<String>,
    pub bootstrap_server_url: Option<String>,
    pub memproofs: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct StaticContentParametersDto {}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct WebBridgeParametersDto {}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub enum ManifestTypeDto {
    HolochainDht(Box<HolochainDhtParametersDto>),
    StaticContent(Box<StaticContentParametersDto>),
    WebBridge(Box<WebBridgeParametersDto>),
}
pub fn manifest_type_from_dto(
    manifest_type_dto: ManifestTypeDto,
) -> Result<schemas::manifest::ManifestType> {
    let dto = match manifest_type_dto {
        ManifestTypeDto::HolochainDht(parameters) => schemas::manifest::ManifestType::HolochainDht(
            Box::new(schemas::manifest::HolochainDhtParameters {
                happ_binary: schemas::manifest::HappBinaryFormat::HappBinaryBlake3Hash(
                    parameters.blob_object_id,
                ),
                holochain_version: parameters.holochain_version,
                holochain_feature_flags: parameters.holochain_feature_flags,
                stun_server_urls: parameters.stun_server_urls.map(|urls| {
                    urls.iter()
                        .map(|url| Url::parse(url).unwrap())
                        .collect::<Vec<Url>>()
                }),
                bootstrap_server_url: match parameters.bootstrap_server_url.clone() {
                    Some(url) => match Url::parse(&url) {
                        Ok(parsed_url) => Some(parsed_url),
                        Err(_) => {
                            return Err(anyhow!("Invalid URL for bootstrap server".to_string()));
                        }
                    },
                    None => None,
                },
                signal_server_url: match parameters.signal_server_url.clone() {
                    Some(url) => match Url::parse(&url) {
                        Ok(parsed_url) => Some(parsed_url),
                        Err(_) => {
                            return Err(anyhow!("Invalid URL for signal server".to_string()));
                        }
                    },
                    None => None,
                },
                memproofs: parameters.memproofs,
            }),
        ),
        ManifestTypeDto::StaticContent(_) => schemas::manifest::ManifestType::StaticContent(
            Box::new(schemas::manifest::StaticContentParameters {}),
        ),
        ManifestTypeDto::WebBridge(_) => schemas::manifest::ManifestType::WebBridge(Box::new(
            schemas::manifest::WebBridgeParameters {},
        )),
    };

    Ok(dto)
}
pub fn workload_type_to_dto(workload_type: schemas::manifest::ManifestType) -> ManifestTypeDto {
    match workload_type {
        schemas::manifest::ManifestType::HolochainDht(parameters) => {
            let blob_id = match parameters.happ_binary {
                schemas::manifest::HappBinaryFormat::HappBinaryBlake3Hash(hash) => hash,
                _ => "Unexpected type".to_string(),
            };

            ManifestTypeDto::HolochainDht(Box::new(HolochainDhtParametersDto {
                blob_object_id: blob_id,
                holochain_feature_flags: parameters.holochain_feature_flags,
                holochain_version: parameters.holochain_version,
                stun_server_urls: parameters
                    .stun_server_urls
                    .clone()
                    .map(|urls| urls.into_iter().map(|url| url.to_string()).collect()),
                bootstrap_server_url: parameters
                    .bootstrap_server_url
                    .clone()
                    .map(|url| url.to_string()),
                signal_server_url: parameters
                    .signal_server_url
                    .clone()
                    .map(|url| url.to_string()),
                memproofs: parameters.memproofs,
            }))
        }
        schemas::manifest::ManifestType::StaticContent(_) => {
            ManifestTypeDto::StaticContent(Box::new(StaticContentParametersDto {}))
        }
        schemas::manifest::ManifestType::WebBridge(_) => {
            ManifestTypeDto::WebBridge(Box::new(WebBridgeParametersDto {}))
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct CreateManifestDto {
    pub name: String,
    pub tag: Option<String>,
    pub manifest_type: ManifestTypeDto,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct ManifestDto {
    pub id: String,
    pub name: String,
    pub tag: Option<String>,
    pub manifest_type: ManifestTypeDto,
}
