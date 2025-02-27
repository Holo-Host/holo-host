use super::mongodb::IntoIndexes;
use anyhow::Result;
use bson::oid::ObjectId;
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
pub use String as PubKey;

// Provide type Alias for SemVer (semantic versioning)
pub use String as SemVer;

// ==================== User Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoleInfo {
    pub collection_id: ObjectId, // Hoster/Developer colleciton Mongodb ID ref
    pub pubkey: PubKey,          //  Hoster/Developer Pubkey *INDEXED*
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserPermission {
    Admin,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Metadata {
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
    pub created_at: Option<DateTime>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    pub jurisdiction: String,
    pub permissions: Vec<UserPermission>,
    pub user_info_id: Option<ObjectId>, // *INDEXED*
    pub developer: Option<RoleInfo>,    // *INDEXED*
    pub hoster: Option<RoleInfo>,       // *INDEXED*
}

// Indexing for User
impl IntoIndexes for User {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // add user_info_id index
        let user_info_id_index_doc = doc! { "user_info_id": 1 };
        let user_info_id_index_opts = Some(
            IndexOptions::builder()
                .name(Some("user_info_id_index".to_string()))
                .build(),
        );
        indices.push((user_info_id_index_doc, user_info_id_index_opts));

        // add developer index
        let developer_index_doc = doc! { "developer": 1 };
        let developer_index_opts = Some(
            IndexOptions::builder()
                .name(Some("developer_index".to_string()))
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        // add host index
        let host_index_doc = doc! { "hoster": 1 };
        let host_index_opts = Some(
            IndexOptions::builder()
                .name(Some("hoster_index".to_string()))
                .build(),
        );
        indices.push((host_index_doc, host_index_opts));

        Ok(indices)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    pub user_id: ObjectId,
    pub email: String, // *INDEXED*
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
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    pub user_id: ObjectId,
    pub active_workloads: Vec<ObjectId>,
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
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    pub user_id: ObjectId,
    pub assigned_hosts: Vec<ObjectId>,
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
    pub memory: i64, // GiB
    pub disk: i64,   // ssd; GiB
    pub cores: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    pub device_id: PubKey, // *INDEXED*
    pub ip_address: String,
    pub remaining_capacity: Capacity,
    pub avg_uptime: f64,
    pub avg_network_speed: i64,
    pub avg_latency: i64,
    pub assigned_hoster: ObjectId,
    pub assigned_workloads: Vec<ObjectId>,
}

impl IntoIndexes for Host {
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];
        //  Add Device ID Index
        let pubkey_index_doc = doc! { "device_id": 1 };
        let pubkey_index_opts = Some(
            IndexOptions::builder()
                .name(Some("device_id_index".to_string()))
                .build(),
        );
        indices.push((pubkey_index_doc, pubkey_index_opts));
        Ok(indices)
    }
}

// ==================== Workload Schema ====================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadState {
    Reported,
    Assigned,
    Pending,
    Installed,
    Running,
    Updating,
    Updated,
    Removed,
    Uninstalled,
    Error(String),   // String = error message
    Unknown(String), // String = context message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub desired: WorkloadState,
    pub actual: WorkloadState,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemSpecs {
    pub capacity: Capacity,
    pub avg_network_speed: i64, // Mbps
    pub avg_uptime: f64, //  decimal value between 0-1 representing avg uptime over past month
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub metadata: Metadata,
    pub assigned_developer: ObjectId, // *INDEXED*
    pub version: SemVer,
    pub nix_pkg: String, // (Includes everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
    pub min_hosts: i32,
    pub system_specs: SystemSpecs,
    pub assigned_hosts: Vec<ObjectId>,
    pub status: WorkloadStatus,
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
            metadata: Metadata {
                is_deleted: false,
                created_at: Some(DateTime::now()),
                updated_at: Some(DateTime::now()),
                deleted_at: None,
            },
            version: semver,
            nix_pkg: String::new(),
            assigned_developer: ObjectId::new(),
            min_hosts: 1,
            system_specs: SystemSpecs {
                capacity: Capacity {
                    memory: 64,
                    disk: 400,
                    cores: 20,
                },
                avg_network_speed: 200,
                avg_uptime: 0.8,
            },
            assigned_hosts: Vec::new(),
            status: WorkloadStatus {
                id: None, // skips serialization when `None`
                desired: WorkloadState::Unknown("default state".to_string()),
                actual: WorkloadState::Unknown("default state".to_string()),
            },
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
