use crate::mongodb::traits::{IntoIndexes, MutMetadata};

use super::metadata::Metadata;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;

pub const JOB_COLLECTION_NAME: &str = "job";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Db(DbStates),
    Host(HostStates),
    Error(String),
}

impl Default for JobState {
    fn default() -> Self {
        JobState::Db(DbStates::default())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobChangeRequest {
    Update,
    Pause,
    Stop,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DbStates {
    #[default]
    /// Job is created in the db, it is not installed on host
    Created,
    /// Job change has been requested in db
    Requested(JobChangeRequest),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HappStorageState {
    HappInstalled,
    HappNotFound,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostStates {
    /// Job is pending installation (or re-installation after update) on host
    Pending,
    /// Job is running
    Running(HappStorageState),
    /// Job is paused << follow-up with team
    Paused(HappStorageState),
    /// Job is stopped (on host)
    Stopped(HappStorageState),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Job {
    pub _id: ObjectId,
    pub metadata: Metadata,

    /// the developer that owns the job
    pub owner: ObjectId,
    /// the workload that this job is part of
    pub workload: ObjectId,
    /// the host this job is running on
    pub host: ObjectId,
    /// desired state of the job
    pub desired_state: JobState,
    /// current state of the job
    pub current_state: JobState,
    /// additional information about the job set by the service
    pub payload: Option<bson::Document>,
}

impl IntoIndexes for Job {
    fn into_indices(self) -> anyhow::Result<Vec<(Document, Option<IndexOptions>)>> {
        let mut indices = vec![];

        // Create an index on the owner field
        let owner_index = bson::doc! {
            "owner": 1,
        };
        let owner_index_options = IndexOptions::builder()
            .name("owner_index".to_string())
            .build();
        indices.push((owner_index, Some(owner_index_options)));

        // Create an index on the workload field
        let workload_index = bson::doc! {
            "workload": 1,
        };
        let workload_index_options = IndexOptions::builder()
            .name("workload_index".to_string())
            .build();
        indices.push((workload_index, Some(workload_index_options)));

        // Create an index on the host field
        let host_index = bson::doc! {
            "host": 1,
        };
        let host_index_options = IndexOptions::builder()
            .name("host_index".to_string())
            .build();
        indices.push((host_index, Some(host_index_options)));

        Ok(indices)
    }
}

impl MutMetadata for Job {
    fn mut_metadata(&mut self) -> &mut Metadata {
        &mut self.metadata
    }
}
