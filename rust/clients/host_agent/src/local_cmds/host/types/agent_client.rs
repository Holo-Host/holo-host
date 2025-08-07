use crate::local_cmds::host::errors::HostAgentResult;

use async_trait::async_trait;
use nats_utils::types::DeServerAddr;

#[async_trait]
pub trait HostClient {
    type Output;
    async fn start(config: &HostClientConfig) -> HostAgentResult<Self::Output>;
    async fn stop(&self) -> HostAgentResult<()>;
}

#[derive(Clone, Debug)]
pub enum ClientType {
    HostAgent(HostDArgs),
    // HostAuth(..),
}

#[derive(Clone, Debug)]
pub struct HostDArgs {
    pub nats_url: DeServerAddr,
}

#[derive(Clone, Debug)]
pub struct HostClientConfig {
    pub device_id: String,
    pub type_args: ClientType,
}

impl HostClientConfig {
    pub fn new(device_id: &str, client_type: ClientType) -> HostAgentResult<Self> {
        log::debug!("device_id : {device_id}");
        log::debug!("type_args : {client_type:?}");

        let host_client_config = Self {
            device_id: device_id.to_string(),
            type_args: client_type,
        };

        Ok(host_client_config)
    }
}
