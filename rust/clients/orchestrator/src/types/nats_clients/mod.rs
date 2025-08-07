pub mod admin;
pub mod auth;

use crate::{errors::OrchestratorError, types::config::OrchestratorConfig};

pub trait OrchestratorClient {
    type Output;
    async fn start(config: &OrchestratorConfig) -> Result<Self::Output, OrchestratorError>;
    async fn stop(&self) -> Result<(), OrchestratorError>;
}
