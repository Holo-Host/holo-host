use super::mongodb::IntoIndexes;
use anyhow::Result;
use bson::{self, doc, Document};
use mongodb::options::IndexOptions;
use semver::{BuildMetadata, Prerelease};
use serde::{Deserialize, Serialize};

pub const DATABASE_NAME: &str = "holo-hosting";
pub const HOST_COLLECTION_NAME: &str = "host";
pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

// ==================== Host Schema ====================
pub use String as HosterPubKey;

// Provide type Alias for Host, as sometimes the use of "Node" is clearer
pub use Host as Node;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    pub _id: String,                   // Mongodb ID
    pub assigned_hoster: HosterPubKey, // *INDEXED*, Hoster pubkey
}

impl IntoIndexes for Host {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Hoster Index
        let hoster_index_doc = doc! { "assigned_hoster": 1 };
        let hoster_index_opts = Some(
            IndexOptions::builder()
                .name(Some("assigned_hoster_index".to_string()))
                .build(),
        );
        indices.push((hoster_index_doc, hoster_index_opts));

        Ok(indices)
    }
}

// ==================== Workload Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct HolochainEnv {
    pub overlay_network: String,
    pub keystore_service_address: String,
    pub membrane_proof: Option<String>,
    pub network_seed: Option<String>,
    pub ui_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BaseEnv {
    pub overlay_network: Option<String>,
    pub keystore_service_address: Option<String>,
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Environment {
    Holochain(HolochainEnv),
    Baseline(BaseEnv),
}

impl Default for Environment {
    fn default() -> Self {
        Environment::Baseline(BaseEnv::default())
    }
}

pub use String as SemVer;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    pub _id: String, // Mongodb ID
    pub version: SemVer,
    pub file: url::Url,              // (eg: DNA URL, wasm bin url)
    pub assigned_hosts: Vec<String>, // Host Device IDs (eg: mac_id)
}

impl Default for Workload {
    fn default() -> Self {
        let version = semver::Version {
            major: 0,
            minor: 0,
            patch: 1,
            pre: Prerelease::EMPTY,
            build: BuildMetadata::EMPTY,
        };

        let semver = version.to_string();

        Self {
            _id: String::new(),
            version: semver,
            file: url::Url::parse("http://localhost").expect("Default URL should always be valid"),
            assigned_hosts: Vec::new(),
        }
    }
}

impl IntoIndexes for Workload {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Email Index
        let developer_index_doc = doc! { "assigned_developer": 1 };
        let developer_index_opts = Some(
            IndexOptions::builder()
                .unique(true)
                .name(Some("assigned_developer_index".to_string()))
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        Ok(indices)
    }
}
