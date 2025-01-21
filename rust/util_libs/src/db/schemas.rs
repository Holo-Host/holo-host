use super::mongodb::IntoIndexes;
use anyhow::Result;
use bson::{self, doc, DateTime, Document};
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
pub use bson::oid::ObjectId as MongoDbId;

// ==================== User Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserPermission {
    Admin
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub jurisdiction: String,
    pub permissions: Vec<UserPermission>,
    pub user_info: Option<MongoDbId>,
    pub developer: Option<MongoDbId>,
    pub host: Option<MongoDbId>,

    // base
    pub deleted: bool,
    pub deleted_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub created_at: Option<DateTime>,
}

// No Additional Indexing for Developer
impl IntoIndexes for User {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // add user_info index
        let user_info_index_doc = doc! { "user_info": 1 };
        let user_info_index_opts = Some(
            IndexOptions::builder()
                .name(Some("user_info_index".to_string()))
                .build(),
        );
        indices.push((user_info_index_doc, user_info_index_opts));

        // add developer index
        let developer_index_doc = doc! { "developer": 1 };
        let developer_index_opts = Some(
            IndexOptions::builder()
                .name(Some("developer_index".to_string()))
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        // add host index
        let host_index_doc = doc! { "host": 1 };
        let host_index_opts = Some(
            IndexOptions::builder()
                .name(Some("host_index".to_string()))
                .build(),
        );
        indices.push((host_index_doc, host_index_opts));

        Ok(indices)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserInfo  {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub user: MongoDbId,
    pub email: String,
    pub given_names: String,
    pub family_name: String,
}

impl IntoIndexes for UserInfo {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // add email index
        let email_index_doc = doc! { "email": 1 };
        let email_index_opts = Some(
            IndexOptions::builder()
                .name(Some("email_index".to_string()))
                .build(),
        );
        indices.push((email_index_doc, email_index_opts));

        Ok(indices)
    }
}

// ==================== Developer Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Developer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<MongoDbId>,
    pub user_id: String, // MongoDB ID ref to `user._id` (which stores the hoster's pubkey, jurisdiction and email)
    pub active_workloads: Vec<MongoDbId>, // MongoDB ID refs to `workload._id`
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
    pub assigned_hosts: Vec<MongoDbId>, // MongoDB ID refs to `host._id`
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
    pub assigned_hoster: MongoDbId,
    pub device_id: String, // *INDEXED*, Auto-generated Nats server ID
    pub ip_address: String,
    pub remaining_capacity: Capacity,
    pub avg_uptime: i64,
    pub avg_network_speed: i64,
    pub avg_latency: i64,
    pub assigned_workloads: Vec<String>, // MongoDB ID refs to `workload._id`
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
    Updating,
    Error(String), // String = error message
    Unknown(String), // String = context message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadStatus {
    pub id: Option<String>, 
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
    pub assigned_developer: MongoDbId, // *INDEXED*, Developer Mongodb ID
    pub version: SemVer,
    pub nix_pkg: String, // (Includes everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
    pub min_hosts: u16,
    pub system_specs: SystemSpecs,
    pub assigned_hosts: Vec<MongoDbId>, // Host Device IDs (eg: assigned nats server id)
    pub deleted: bool,
    pub deleted_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
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
            assigned_developer: MongoDbId::new(),
            min_hosts: 1,
            system_specs: SystemSpecs {
                capacity: Capacity {
                    memory: 64,
                    disk: 400,
                    cores: 20
                }
            },
            assigned_hosts: Vec::new(),
            deleted: false,
            deleted_at: None,
            updated_at: None,
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
