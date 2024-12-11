use super::mongodb::IntoIndexes;
use anyhow::Result;
use bson::{self, doc, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

// ==================== User Schema ====================
#[derive(Serialize, Deserialize, Debug)]
pub enum Role {
    Developer(String), // jwt string
    Host(String),      // host pubkey
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RoleInfo {
    pub id: String, // Hoster/Developer Mongodb ID
    pub role: Role, // *INDEXED*
}

#[derive(Serialize, Deserialize, Debug)]
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
                .unique(true) // Enforce uniqueness
                .name(Some("email_index".to_string())) // Name of the index
                .build(),
        );
        indices.push((email_index_doc, email_index_opts));

        // Add Role Index
        let role_index_doc = doc! { "roles.role": 1 };
        let role_index_opts = Some(
            IndexOptions::builder()
                .unique(true) // Enforce uniqueness
                .name(Some("role_index".to_string())) // Name of the index
                .build(),
        );
        indices.push((role_index_doc, role_index_opts));

        Ok(indices)
    }
}

// ==================== Developer Schema ====================
#[derive(Serialize, Deserialize, Debug)]
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

// ==================== Hoster Schema ====================
#[derive(Serialize, Deserialize, Debug)]
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

// ==================== Host Schema ====================

#[derive(Serialize, Deserialize, Debug)]
pub struct VM {
    pub port: u16,
    pub size: u64,
    pub agent_pubkey: String,
}

// Provide type Alias for Host, as sometimes the use of "Node" is clearer
pub use Host as Node;

#[derive(Serialize, Deserialize, Debug)]
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
                .unique(true) // Enforce uniqueness
                .name(Some("remaining_capacity_index".to_string())) // Name of the index
                .build(),
        );
        indices.push((capacity_index_doc, capacity_index_opts));

        Ok(indices)
    }
}

// ==================== Workload Schema ====================
#[derive(Serialize, Deserialize, Debug)]
pub struct HolochainEnv {
    pub overlay_network: String,
    pub keystore_service_address: String,
    pub membrane_proof: Option<String>,
    pub network_seed: Option<String>,
    pub ui_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BaseEnv {
    pub overlay_network: Option<String>,
    pub keystore_service_address: Option<String>,
    pub size: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Environment {
    Holochain(HolochainEnv),
    Baseline(BaseEnv),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Workload {
    pub _id: String,    // Mongodb ID
    pub file: url::Url, // (eg: DNA URL, wasm bin url)
    pub env: Environment,
    pub assigned_developer: String, // *INDEXED*, Developer Mongodb ID
    pub min_hosts: u16,
    pub assigned_hosts: Vec<String>, // Host Device IDs (eg: mac_id)
}

// No Additional Indexing for Hoster
impl IntoIndexes for Workload {
    fn into_indices(&self) -> Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        //  Add Email Index
        let developer_index_doc = doc! { "assigned_developer": 1 };
        let developer_index_opts = Some(
            IndexOptions::builder()
                .unique(true) // Enforce uniqueness
                .name(Some("assigned_developer_index".to_string())) // Name of the index
                .build(),
        );
        indices.push((developer_index_doc, developer_index_opts));

        Ok(indices)
    }
}
