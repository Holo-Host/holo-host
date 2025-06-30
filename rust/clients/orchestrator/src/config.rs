use crate::{Args, errors::OrchestratorError};

use anyhow::Result;
use db_utils::mongodb::get_mongodb_url;
use nats_utils::jetstream_client::get_nats_creds_by_nsc;
use nats_utils::types::NatsRemoteArgs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    pub nats_remote_args: NatsRemoteArgs,
    pub mongo_uri: String,
    pub auth_creds_path: PathBuf,
    pub admin_creds_path: PathBuf,
    pub _skip_tls_verification: bool,
}

impl OrchestratorConfig {
    pub fn from_args(args: Args) -> Result<Self, OrchestratorError> {
        let mongo_uri = get_mongodb_url();
        let auth_creds_path = PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "AUTH", "orchestrator_auth"))
            .map_err(|e| OrchestratorError::Configuration(format!("Invalid auth creds path: {}", e)))?;
        let admin_creds_path = PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "ADMIN", "admin"))
            .map_err(|e| OrchestratorError::Configuration(format!("Invalid admin creds path: {}", e)))?;
        let skip_tls_verification = args.nats_remote_args.nats_skip_tls_verification_danger;

        Ok(Self {
            nats_remote_args: args.nats_remote_args,
            mongo_uri,
            auth_creds_path,
            admin_creds_path,
            _skip_tls_verification: skip_tls_verification,
        })
    }
}
