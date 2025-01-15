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

// Provide type Alias for HosterPubKey
pub use String as HosterPubKey;

// Provide type Alias for DeveloperPubkey
pub use String as DeveloperPubkey;

// Provide type Alias for DeveloperJWT
pub use String as DeveloperJWT;

// Provide type Alias for SemVer (semantic versioning)
pub use String as SemVer;

// Providetype Alias for MongoDB ID (mongo's automated id)
pub use String as MongoDbId;

// ==================== User Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Role {
    Developer(DeveloperJWT), // jwt string
    Host(HosterPubKey),      // host pubkey
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoleInfo {
    pub ref_id: String, // Hoster/Developer Mongodb ID ref
    pub role: Role,     // *INDEXED*
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub email: String,
    pub jurisdiction: String,
    pub roles: Vec<RoleInfo>,
}

impl IntoIndexes for User {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub user_id: String, // MongoDB ID ref to `user._id` (which stores the hoster's pubkey, jurisdiction and email)
    pub requested_workloads: Vec<String>, // MongoDB ID refs to `workload._id`
}

// No Additional Indexing for Developer
impl IntoIndexes for Developer {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

// ==================== Hoster Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Hoster {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub user_id: String, // MongoDB ID ref to `user.id` (which stores the hoster's pubkey, jurisdiction and email)
    pub assigned_hosts: Vec<String>, // Auto-generated Nats server IDs
}

// No Additional Indexing for Hoster
impl IntoIndexes for Hoster {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

// ==================== Host Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Capacity {
    pub memory: i64,  // GiB
    pub disk: i64,  // ssd; GiB 
    pub cores: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub device_id: String, // *INDEXED*, Auto-generated Nats server ID
    pub ip_address: String,
    pub remaining_capacity: Capacity,
    pub avg_uptime: i64,
    pub avg_network_speed: i64,
    pub avg_latency: i64,
    pub assigned_workloads: Vec<String>, // MongoDB ID refs to `workload._id`
    pub assigned_hoster: HosterPubKey,   // *INDEXED*, Hoster pubkey
}

impl IntoIndexes for Host {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadState {
    Reported,
    Assigned, // String = host id
    Pending,
    Installed,
    Running,
    Removed,
    Uninstalled,
    Error(String), // String = error message
    Unknown(String), // String = context message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadStatus {
    pub desired: WorkloadState,
    pub actual: WorkloadState,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemSpecs {
    pub capacity: Capacity
    // network_speed: i64
    // uptime: i64
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub version: SemVer,
    pub nix_pkg: String, // (Includes everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
    pub assigned_developer: String, // *INDEXED*, Developer Mongodb ID
    pub min_hosts: u16,
    pub system_specs: SystemSpecs,
    pub assigned_hosts: Vec<String>, // Host Device IDs (eg: assigned nats server id)
    // pub status: WorkloadStatus,
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
            _id: None,
            version: semver,
            nix_pkg: String::new(),
            assigned_developer: String::new(),
            min_hosts: 1,
            system_specs: SystemSpecs {
                capacity: Capacity {
                    memory: 64,
                    disk: 400,
                    cores: 20
                }
            },
            assigned_hosts: Vec::new(),
        }
    }
}

impl IntoIndexes for Workload {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
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
