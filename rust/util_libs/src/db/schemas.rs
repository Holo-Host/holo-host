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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    pub _id: String,                   // Mongodb ID
    pub device_id: String,             // *INDEXED*, Auto-generated Nats server ID
    pub assigned_hoster: HosterPubKey, // Hoster pubkey
}

impl IntoIndexes for Host {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Device ID Index
        let device_id_index_doc = doc! { "device_id": 1 };
        let device_id_index_opts = Some(
            IndexOptions::builder()
                .name(Some("device_id_index".to_string()))
                .build(),
        );
        indices.push((device_id_index_doc, device_id_index_opts));

        Ok(indices)
    }
}

// ==================== Workload Schema ====================
pub use String as SemVer;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    pub _id: String, // Mongodb ID
    pub version: SemVer,
    pub nix_pkg: String, // (Includes everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
    pub assigned_hosts: Vec<String>, // Host Device IDs (eg: mac_id)
}

impl Default for Workload {
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
            _id: String::new(),
            version: semver,
            nix_pkg: String::new(),
            assigned_hosts: Vec::new(),
        }
    }
}

impl IntoIndexes for Workload {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Developer Index
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
