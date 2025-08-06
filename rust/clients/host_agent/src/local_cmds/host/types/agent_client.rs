use crate::local_cmds::host::errors::HostAgentResult;

use async_trait::async_trait;
use nats_utils::types::DeServerAddr;

use std::path::PathBuf;

#[async_trait]
pub trait HostClient {
    type Output;
    async fn start(config: &HostClientConfig) -> HostAgentResult<Self::Output>;
    async fn stop(&self) -> HostAgentResult<()>;
}

#[derive(Clone, Debug)]
pub enum ClientType {
    HostAuth(HostAuthArgs),
    HostAgent(HostDArgs),
}

#[derive(Clone, Debug)]
pub struct HostAuthArgs {
    pub hub_url: String,
}

#[derive(Clone, Debug)]
pub struct HostDArgs {
    pub nats_url: DeServerAddr,
}

#[derive(Clone, Debug)]
pub struct HostClientConfig {
    pub nats_creds_path: PathBuf,
    pub device_id: String,
    pub type_args: TypeSpecificArgs,
}

#[derive(Clone, Debug)]
pub enum TypeSpecificArgs {
    HostAgent(HostDArgs),
    // HostAuth(..),
}
