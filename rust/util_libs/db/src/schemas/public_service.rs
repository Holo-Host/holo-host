use anyhow::Result;
use bson::{oid::ObjectId, DateTime, Document};
use mongodb::options::IndexOptions;
use serde::{Deserialize, Serialize};

use super::metadata::Metadata;
use crate::mongodb::traits::{IntoIndexes, MutMetadata};

/// Current schema version. Used internally and monotonically increases.
pub const SCHEMA_VERSION: i32 = 1;

/// Collection for tracking public services and their public IPs
pub const PUBLIC_SERVICE_COLLECTION_NAME: &str = "public_services";

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
    pub _id: ObjectId,
    /// Schema version.
    pub schema_version: i32,
    /// Service Type
    pub service_type: PublicServiceType,
    /// Common metadata fields
    pub metadata: Metadata,
    /// List of records
    pub records: Vec<ServiceRecord>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServiceRecord {
    /// DNS name to associate with service
    pub service_name: String,
    /// public IPv6 addresses the service is available on.
    pub aaaa_addrs: Vec<String>,
    /// public IPv4 addresses the service is available on.
    pub a_addrs: Vec<String>,
    /// FQDNs for CNAMES
    pub cname_addrs: Vec<String>,
    /// FQDNs for answers to NS record questions.
    pub ns_addrs: Vec<String>,
}

/// Default implementation for PublicService to help initialise a few fields.
impl Default for PublicService {
    fn default() -> Self {
        Self {
            _id: ObjectId::new(),
            schema_version: SCHEMA_VERSION,
            service_type: PublicServiceType::Default,
            metadata: Metadata {
                is_deleted: false,
                created_at: Some(DateTime::now()),
                updated_at: Some(DateTime::now()),
                deleted_at: None,
            },
            records: vec![],
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
