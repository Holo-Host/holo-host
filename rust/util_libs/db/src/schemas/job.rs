use crate::mongodb::traits::{IntoIndexes, MutMetadata};

use super::metadata::Metadata;
use bson::{oid::ObjectId, Document};
use mongodb::options::IndexOptions;

pub const JOB_COLLECTION_NAME: &str = "job";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    #[default]
    /// Job is created in the db, it is not installed on host
    Created,
    /// Job is pending installation on host
    Pending,
    /// Job has been installed on host
    Installed,
    /// Job is running
    Running,
    /// Job is paused
    Paused,
    /// Job is stopped
    Stopped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Job {
    pub _id: Option<ObjectId>,
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