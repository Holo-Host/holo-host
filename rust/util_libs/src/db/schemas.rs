use super::mongodb::IntoIndexes;
use anyhow::Result;
use bson::{self, doc, Document};
use mongodb::options::IndexOptions;
use semver::{BuildMetadata, Prerelease};
use serde::{Deserialize, Serialize};

pub const DATABASE_NAME: &str = "holo-hosting";
pub const USER_COLLECTION_NAME: &str = "user";
pub const DEVELOPER_COLLECTION_NAME: &str = "developer";
pub const HOSTER_COLLECTION_NAME: &str = "hoster";
pub const HOST_COLLECTION_NAME: &str = "host";
pub const WORKLOAD_COLLECTION_NAME: &str = "workload";

// ==================== User Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Role {
    Developer(String), // jwt string
    Host(String),      // host pubkey
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoleInfo {
    pub id: String, // Hoster/Developer Mongodb ID
    pub role: Role, // *INDEXED*
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    pub _id: String, // Mongodb ID (automated default)
    pub email: String,
    pub jurisdiction: String,
    pub roles: Vec<RoleInfo>,
}

impl IntoIndexes for User {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Email Index
        let email_index_doc = doc! { "email": 1 };
        let email_index_opts = Some(
            IndexOptions::builder()
                .unique(true)
                .name(Some("email_index".to_string()))
                .build(),
        );
        indices.push((email_index_doc, email_index_opts));

        // Add Role Index
        let role_index_doc = doc! { "roles.role": 1 };
        let role_index_opts = Some(
            IndexOptions::builder()
                .name(Some("role_index".to_string()))
                .build(),
        );
        indices.push((role_index_doc, role_index_opts));

        Ok(indices)
    }
}

// ==================== Developer Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Developer {
    pub _id: String,                      // Mongodb ID (automated default)
    pub user_id: String, // MongoDB ID ref to `user._id` (which stores the hoster's pubkey, jurisdiction and email)
    pub requested_workloads: Vec<String>, // MongoDB ID refs to `workload._id`
}

// No Additional Indexing for Developer
impl IntoIndexes for Developer {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

// ==================== Hoster Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Hoster {
    pub _id: String,                 // Mongodb ID (automated default)
    pub user_id: String, // MongoDB ID ref to `user.id` (which stores the hoster's pubkey, jurisdiction and email)
    pub assigned_hosts: Vec<String>, // Auto-generated Nats server IDs
}

// No Additional Indexing for Hoster
impl IntoIndexes for Hoster {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

// ==================== Host Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct VM {
    pub port: u16,
    pub size: u64,
    pub agent_pubkey: String,
}

// Provide type Alias for HosterPubKey
pub use String as HosterPubKey;

// Provide type Alias for Host
pub use Host as Node;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    pub _id: String,            // Mongodb ID (automated default)
    pub device_id: Vec<String>, // Auto-generated Nats server ID
    pub ip_address: String,
    pub remaining_capacity: u64,
    pub avg_uptime: u64,
    pub avg_network_speed: u64,
    pub avg_latency: u64,
    pub vms: Vec<VM>,
    pub assigned_workloads: Vec<String>, // MongoDB ID refs to `workload._id`
    pub assigned_hoster: String,         // *INDEXED*, Hoster pubkey
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
pub use String as SemVer;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    pub _id: String, // Mongodb ID (automated default)
    pub version: SemVer,
    pub nix_pkg: String, // (Includes everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
    pub assigned_developer: String, // *INDEXED*, Developer Mongodb ID
    pub min_hosts: u16,
    pub assigned_hosts: Vec<String>, // Host Device IDs (eg: assigned nats server id)
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
            nix_pkg: String::new(),
            assigned_developer: String::new(),
            min_hosts: 1,
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
                .name(Some("assigned_developer_index".to_string()))
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        Ok(indices)
    }
}
