/// Database schemas and types for the Holo Hosting system.
///
/// This module defines the schema structures and their MongoDB index configurations
/// for the Holo Hosting database. It includes schemas for users, developers, hosters,
/// hosts, and workloads, along with their associated types and relationships.
///
/// # Examples
///
/// ```rust,no_run
/// use db_utils::schemas::{User, Workload, DATABASE_NAME};
/// use mongodb::Client;
///
/// // Work with collections using the defined schemas
/// async fn example() -> Result<(), anyhow::Error> {
///     let client = Client::with_uri_str("mongodb://localhost:27017").await?;
///
///     // Set up db and collections with the MongoCollection interface
///     use db_utils::mongodb::MongoCollection;
///     let users = MongoCollection::<User>::new(&client, DATABASE_NAME, "user").await?;
///     let workloads = MongoCollection::<Workload>::new(&client, DATABASE_NAME, "workload").await?;
///
///     Ok(())
/// }
/// ```
///
use super::mongodb::IntoIndexes;
use crate::mongodb::MutMetadata;
use anyhow::Result;
use bson::oid::ObjectId;
use bson::{self, doc, Bson, DateTime, Document};
use hpos_hal::inventory::HoloInventory;
use mongodb::options::IndexOptions;
use semver::{BuildMetadata, Prerelease};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use strum::{EnumDiscriminants, EnumString, FromRepr};
use strum_macros::AsRefStr;
use url::Url;

/// Name of the main database for the Holo Hosting system
pub const DATABASE_NAME: &str = "holo-hosting";
/// Collection name for user documents
pub const USER_COLLECTION_NAME: &str = "user";
/// Collection name for developer documents
pub const DEVELOPER_COLLECTION_NAME: &str = "developer";
/// Collection name for hoster documents
pub const HOSTER_COLLECTION_NAME: &str = "hoster";
/// Collection name for host documents
pub const HOST_COLLECTION_NAME: &str = "host";
/// Collection name for workload documents
pub const WORKLOAD_COLLECTION_NAME: &str = "workload";
/// Collection for tracking public services and their public IPs
pub const PUBLIC_SERVICE_COLLECTION_NAME: &str = "public_services";

/// Type alias for public keys used in the system
pub use String as PubKey;
/// Type alias for semantic version strings
pub use String as SemVer;

/// Information about a user's role (hoster or developer) in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoleInfo {
    /// MongoDB ObjectId reference to the role collection (hoster/developer)
    pub collection_id: ObjectId,
    /// Public key associated with the role
    pub pubkey: PubKey,
}

/// Enumeration of possible user permission levels
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserPermission {
    /// Administrator level permissions
    Admin,
}

/// Common metadata fields for database documents
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Metadata {
    /// Flag indicating if the document has been marked as deleted
    pub is_deleted: bool,
    /// Timestamp when the document was deleted
    pub deleted_at: Option<DateTime>,
    /// Timestamp of the last update
    pub updated_at: Option<DateTime>,
    /// Timestamp when the document was created
    pub created_at: Option<DateTime>,
}

/// User document schema representing a user in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct User {
    /// MongoDB ObjectId of the user document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// User's jurisdiction
    pub jurisdiction: String,
    /// List of user permissions
    pub permissions: Vec<UserPermission>,
    /// Reference to additional user information
    pub user_info_id: Option<ObjectId>,
    /// Developer role information if user is a developer
    pub developer: Option<RoleInfo>,
    /// Hoster role information if user is a hoster
    pub hoster: Option<RoleInfo>,
}
impl IntoIndexes for User {
    /// Defines MongoDB indices for the User collection
    ///
    /// Creates indices for:
    /// - user_info_id
    /// - developer role
    /// - hoster role
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

impl MutMetadata for User {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

/// Additional user information schema
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserInfo {
    /// MongoDB ObjectId of the user info document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub user_id: ObjectId,
    /// User's email address
    pub email: String,
    /// User's given names
    pub given_names: String,
    /// User's family name
    pub family_name: String,
}

impl IntoIndexes for UserInfo {
    /// Defines MongoDB indices for the UserInfo collection
    ///
    /// Creates an index for:
    /// - email address
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

impl MutMetadata for UserInfo {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

/// Developer document schema representing a developer in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Developer {
    /// MongoDB ObjectId of the developer document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub user_id: ObjectId,
    /// List of workloads created by this developer
    pub active_workloads: Vec<ObjectId>,
}

impl IntoIndexes for Developer {
    /// No additional indices defined for Developer collection
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl MutMetadata for Developer {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

/// Hoster document schema representing a hoster in the system
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Hoster {
    /// MongoDB ObjectId of the hoster document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the associated user
    pub user_id: ObjectId,
    /// List of hosts managed by this hoster
    pub assigned_hosts: Vec<ObjectId>,
}

impl IntoIndexes for Hoster {
    /// No additional indices defined for Hoster collection
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl MutMetadata for Hoster {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

/// Host document schema representing a hosting device in the system
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Host {
    /// MongoDB ObjectId of the host document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Unique identifier for the device
    pub device_id: String,
    /// Hardware inventory information
    pub inventory: HoloInventory,
    /// Average uptime as a percentage
    pub avg_uptime: f64,
    /// Average network speed in Mbps
    pub avg_network_speed: i64,
    /// Average latency in milliseconds
    pub avg_latency: i64,
    /// IP address of the host
    pub ip_address: Option<String>,
    /// Reference to the assigned hoster
    pub assigned_hoster: Option<ObjectId>,
    /// List of workloads running on this host
    pub assigned_workloads: Vec<ObjectId>,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            _id: None,
            metadata: Metadata::default(),
            device_id: Default::default(),
            inventory: HoloInventory::default(),
            avg_uptime: 100.00,     // Start with full 100% uptime
            avg_network_speed: 100, // Start at decent network speed (mbps)
            avg_latency: 100,       // Start at decent latency time
            assigned_workloads: vec![],
            assigned_hoster: None,
            ip_address: None,
        }
    }
}

impl IntoIndexes for Host {
    /// Defines MongoDB indices for the Host collection
    ///
    /// Creates an index for:
    /// - device_id
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

impl MutMetadata for Host {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

/// Enumeration of possible workload states
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, AsRefStr, EnumDiscriminants, FromRepr,
)]
#[strum_discriminants(
    derive(EnumString, Serialize, Deserialize),
    repr(usize),
    strum(serialize_all = "snake_case")
)]
pub enum WorkloadState {
    /// Workload reported by developer
    Reported,
    /// Workload assigned to host
    Assigned,
    /// Workload installation pending on host device
    Pending,
    /// Workload installed on host device
    Installed,
    /// Workload running on host device
    Running,
    /// Workload is being updated
    Updating,
    /// Workload update completed
    Updated,
    /// Workload marked for deletion
    Deleted,
    /// Workload links removed
    Removed,
    /// Workload uninstalled from host device
    Uninstalled,
    /// Error state with message
    Error(String),
    /// Unknown state with context
    Unknown(String),
}

/// Status information for a workload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadStatus {
    /// Optional MongoDB ObjectId for the status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    /// Desired state of the workload
    pub desired: WorkloadState,
    /// Actual current state of the workload
    pub actual: WorkloadState,

    pub payload: WorkloadStatePayload,
}

/// Resource capacity requirements for a workload
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Capacity {
    /// Required drive space in GiB
    pub drive: i64,
    /// Required CPU cores
    pub cores: i64,
}

/// System specifications for a workload
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SystemSpecs {
    /// Resource capacity requirements
    pub capacity: Capacity,
    /// Required network speed in Mbps
    pub avg_network_speed: i64,
    /// Required uptime as a decimal between 0-1
    pub avg_uptime: f64,
}

/// Workload document schema representing a deployable application
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    /// MongoDB ObjectId of the workload document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Reference to the developer who created this workload
    pub assigned_developer: ObjectId,
    /// Semantic version of the workload
    pub version: SemVer,
    /// Minimum number of hosts required
    pub min_hosts: i32,
    /// System requirements for the workload
    pub system_specs: SystemSpecs,
    /// List of hosts this workload is assigned to
    pub assigned_hosts: Vec<ObjectId>,
    /// Current status of the workload
    pub status: WorkloadStatus,
    pub manifest: WorkloadManifest, // (Includes information about everthing needed to deploy workload - ie: binary & env pkg & deps, etc)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WorkloadManifest {
    None,
    ExtraContainerPath { extra_container_path: String },
    ExtraContainerStorePath { store_path: PathBuf },
    ExtraContainerBuildCmd { nix_args: Box<[String]> },
    HolochainDhtV1(Box<WorkloadManifestHolochainDhtV1>),
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
pub enum WorkloadStatePayload {
    #[default]
    None,
    HolochainDhtV1(Bson),
}

#[derive(Serialize, Deserialize, Clone, Debug, clap::Args)]
pub struct WorkloadManifestHolochainDhtV1 {
    #[arg(long, value_delimiter = ',')]
    pub happ_binary_url: Url,
    #[arg(long, value_delimiter = ',')]
    pub network_seed: String,
    #[arg(long, value_delimiter = ',', value_parser = parse_key_val::<String, String>)]
    pub memproof: Option<HashMap<String, String>>,
    #[arg(long, value_delimiter = ',')]
    pub bootstrap_server_url: Option<Url>,
    #[arg(long, value_delimiter = ',')]
    pub signal_server_url: Option<Url>,
    #[arg(long, value_delimiter = ',')]
    pub stun_server_urls: Option<Vec<Url>>,
    #[arg(long, value_delimiter = ',')]
    pub holochain_feature_flags: Option<Vec<String>>,
    #[arg(long, value_delimiter = ',')]
    pub holochain_version: Option<String>,
    #[arg(long)]
    pub http_gw_enable: bool,
    #[arg(long)]
    pub http_gw_allowed_fns: Option<Vec<String>>,
}

/// Parse a single key-value pair
fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

impl Default for Workload {
    /// Creates a default workload configuration with:
    /// - Version 0.0.0
    /// - Minimum 1 host
    /// - 1 GiB drive space
    /// - 1 cores
    /// - 0 Mbps network speed
    /// - 0% uptime requirement
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
            assigned_developer: ObjectId::new(),
            min_hosts: 1,
            system_specs: SystemSpecs {
                capacity: Capacity { drive: 1, cores: 1 },
                avg_network_speed: 0,
                avg_uptime: 0f64,
            },
            assigned_hosts: Vec::new(),
            status: WorkloadStatus {
                id: None,
                desired: WorkloadState::Unknown("default state".to_string()),
                actual: WorkloadState::Unknown("default state".to_string()),
                payload: WorkloadStatePayload::None,
            },
            manifest: WorkloadManifest::None,
        }
    }
}

impl IntoIndexes for Workload {
    /// Defines MongoDB indices for the Workload collection
    ///
    /// Creates an index for:
    /// - assigned_developer
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Assigned Developer Index
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

impl MutMetadata for Workload {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum PublicServiceType {
    Default,
    GatewayServer,
    ApiServer,
}

/// Public Services document schema representing services we host on public IPs. We can maintain
/// this for a few services for now, but as we increase our automation/orchestration for deployment
/// and management of public services, we should have the orchestration maintain this data. At the
/// moment, the only consumer of this data is the Holo DNS service and it does so purely read-only.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PublicService {
    /// MongoDB ObjectId of the workload document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    /// Common metadata fields
    pub metadata: Metadata,
    /// Service Type
    pub service_type: PublicServiceType,
    /// DNS name to associate with service
    pub service_name: String,
    /// public IPv6 addresses the service is available on.
    pub aaaa_addrs: Vec<String>,
    /// public IPv4 addresses the service is available on.
    pub a_addrs: Vec<String>,
}

/// Default implementation for PublicService to help initialise a few fields.
impl Default for PublicService {
    fn default() -> Self {
        Self {
            _id: None,
            metadata: Metadata {
                is_deleted: false,
                created_at: Some(DateTime::now()),
                updated_at: Some(DateTime::now()),
                deleted_at: None,
            },
            service_type: PublicServiceType::Default,
            service_name: "".to_string(),
            aaaa_addrs: vec![],
            a_addrs: vec![],
        }
    }
}

impl IntoIndexes for PublicService {
    /// No additional indices defined for PublicService collection -- it's expected to be a very
    /// small number of documents for the forseeable future.
    fn into_indices(self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl MutMetadata for PublicService {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
