use crate::auth::keys::{AuthCredType, Keys};
use crate::auth::utils::json_to_base64;
use crate::local_cmds::host::errors::HostAgentResult;

use async_trait::async_trait;
use authentication::types::AuthGuardToken;
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
    pub auth_guard_token: AuthGuardToken,
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
    HostAuth(HostAuthClientArgs),
    HostAgent(HostDArgs),
}

#[derive(Clone, Debug)]
pub struct HostAuthClientArgs {
    pub inbox_sub_prefix: String,
    pub token: String,
    pub hub_url: String,
}

impl HostClientConfig {
    pub fn new(
        device_id: &str,
        host_agent_keys: Keys,
        client_type: ClientType,
    ) -> HostAgentResult<Self> {
        let nats_creds_path = match &host_agent_keys.creds {
            AuthCredType::Guard(guard_creds_path) => guard_creds_path.clone(),
            AuthCredType::Authenticated(credential_paths) => {
                credential_paths.host_creds_path.clone()
            }
        };

        log::debug!("host client creds path : {nats_creds_path:?}");
        log::debug!("device_id : {device_id}");

        let type_args = match client_type {
            ClientType::HostAuth(args) => {
                let user_auth_json = serde_json::to_string(&args.auth_guard_token)?;
                let user_auth_token = json_to_base64(&user_auth_json)?;

                TypeSpecificArgs::HostAuth(HostAuthClientArgs {
                    inbox_sub_prefix: host_agent_keys.host_pubkey.to_lowercase(),
                    token: user_auth_token,
                    hub_url: args.hub_url,
                })
            }
            ClientType::HostAgent(args) => TypeSpecificArgs::HostAgent(args),
        };

        log::debug!("type_args : {type_args:?}");
        log::debug!("nats_creds_path : {nats_creds_path:?}");

        let host_client_config = Self {
            nats_creds_path,
            device_id: device_id.to_string(),
            type_args,
        };

        Ok(host_client_config)
    }
}
