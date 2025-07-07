pub mod auth;
pub mod admin;

use crate::{types::config::OrchestratorConfig, errors::OrchestratorError};

pub trait OrchestratorClient {
    type Output;
    async fn start(config: &OrchestratorConfig) -> Result<Self::Output, OrchestratorError>;
    async fn stop(&self) -> Result<(), OrchestratorError>;
    fn name(&self) -> &str;
}
