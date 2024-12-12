use super::mongodb::IntoIndexes;
use anyhow::Result;
use bson::{self, doc, from_document, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;

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
    pub _id: String, // *INDEXED*, Mongodb ID
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
                .unique(true)
                .name(Some("role_index".to_string()))
                .build(),
        );
        indices.push((role_index_doc, role_index_opts));

        Ok(indices)
    }
}

impl Borrow<User> for Document {
    fn borrow(&self) -> &User {
        match from_document::<User>(self.clone()) {
            Ok(u) => Box::leak(Box::new(u)), // Leak the box to return a reference
            Err(e) => panic!("Failed to convert Document to User: {}", e), // Handle deserialization error
        }
    }
}

// ==================== Developer Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Developer {
    pub _id: String,
    pub user_id: String, // MongoDB ID ref to `user.id` (which stores the hoster's pubkey, jurisdiction and email)
    pub requested_workloads: Vec<String>, // MongoDB IDS
}

// No Additional Indexing for Developer
impl IntoIndexes for Developer {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl Borrow<Developer> for Document {
    fn borrow(&self) -> &Developer {
        match from_document::<Developer>(self.clone()) {
            Ok(d) => Box::leak(Box::new(d)), // Leak the box to return a reference
            Err(e) => panic!("Failed to convert Document to Developer: {}", e), // Handle deserialization error
        }
    }
}

// ==================== Hoster Schema ====================
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Hoster {
    pub _id: String,
    pub user_id: String, // MongoDB ID ref to `user.id` (which stores the hoster's pubkey, jurisdiction and email)
    pub assigned_hosts: Vec<String>, // device id (g: mac_address)
}

// No Additional Indexing for Hoster
impl IntoIndexes for Hoster {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        Ok(vec![])
    }
}

impl Borrow<Hoster> for Document {
    fn borrow(&self) -> &Hoster {
        match from_document::<Hoster>(self.clone()) {
            Ok(h) => Box::leak(Box::new(h)), // Leak the box to return a reference
            Err(e) => panic!("Failed to convert Document to Hoster: {}", e), // Handle deserialization error
        }
    }
}

// ==================== Host Schema ====================

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct VM {
    pub port: u16,
    pub size: u64,
    pub agent_pubkey: String,
}

// Provide type Alias for Host, as sometimes the use of "Node" is clearer
pub use Host as Node;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Host {
    pub _id: String,            // Mongodb ID
    pub device_id: Vec<String>, // (eg: mac_address)
    pub ip_address: String,
    pub remaining_capacity: u64, // *INDEXED*,
    pub avg_uptime: u64,
    pub avg_network_speed: u64,
    pub avg_latency: u64,
    pub vms: Vec<VM>,
    pub assigned_workloads: String, // Workload Mongodb ID
    pub assigned_hoster: String,    // *INDEXED*, Hoster pubkey
}

impl IntoIndexes for Host {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Email Index
        let capacity_index_doc = doc! { "remaining_capacity": 1 };
        let capacity_index_opts = Some(
            IndexOptions::builder()
                .unique(true)
                .name(Some("remaining_capacity_index".to_string()))
                .build(),
        );
        indices.push((capacity_index_doc, capacity_index_opts));

        Ok(indices)
    }
}

impl Borrow<Host> for Document {
    fn borrow(&self) -> &Host {
        match from_document::<Host>(self.clone()) {
            Ok(h) => Box::leak(Box::new(h)), // Leak the box to return a reference
            Err(e) => panic!("Failed to convert Document to Host: {}", e), // Handle deserialization error
        }
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
    pub size: Option<String>,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Workload {
    pub _id: String,    // Mongodb ID
    pub file: url::Url, // (eg: DNA URL, wasm bin url)
    pub env: Environment,
    pub assigned_developer: String, // *INDEXED*, Developer Mongodb ID
    pub min_hosts: u16,
    pub assigned_hosts: Vec<String>, // Host Device IDs (eg: mac_id)
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

impl Default for Workload {
    fn default() -> Self {
        Self {
            _id: String::new(),
            file: url::Url::parse("http://localhost").expect("Default URL should always be valid"),
            env: Environment::default(),
            assigned_developer: String::new(),
            min_hosts: 0,
            assigned_hosts: Vec::new(),
        }
    }
}

impl Borrow<Workload> for Document {
    fn borrow(&self) -> &Workload {
        match from_document::<Workload>(self.clone()) {
            Ok(w) => Box::leak(Box::new(w)), // Leak the box to return a reference
            Err(e) => panic!("Failed to convert Document to Workload: {}", e), // Handle deserialization error
        }
    }
}
