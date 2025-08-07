use crate::{errors::OrchestratorError, Args};

use anyhow::{Context, Result};
use db_utils::mongodb::get_mongodb_url;
use mongodb::{options::ClientOptions, Client as MongoDBClient};
use nats_utils::types::NatsRemoteArgs;

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub nats_remote_args: NatsRemoteArgs,
    pub mongo_uri: String,
}

impl OrchestratorConfig {
    pub fn from_args(args: Args) -> Result<Self, OrchestratorError> {
        Ok(Self {
            nats_remote_args: args.nats_remote_args,
            mongo_uri: get_mongodb_url(),
        })
    }

    pub async fn setup_database(&self) -> Result<MongoDBClient, OrchestratorError> {
        log::info!("Connecting to mongodb at {}", self.mongo_uri);
        let db_client_options = ClientOptions::parse(&self.mongo_uri)
            .await
            .context(format!("mongo db client: connecting to {}", self.mongo_uri))
            .map_err(|e| OrchestratorError::Configuration(e.to_string()))?;

        MongoDBClient::with_options(db_client_options).map_err(OrchestratorError::Database)
    }
}
