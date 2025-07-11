/*
This client is associated with the:
    - AUTH account
    - auth guard user

Nb: Once the host and hoster are validated, and the host creds file is created,
...this client should safely close and then the `hostd.workload` manager should spin up.

This client is responsible for:
    - generating new key for host and accessing hoster key from provided config file
    - calling the host auth service to:
        - validate hoster hc pubkey and email
        - send the host pubkey to the orchestrator to register with the orchestrator key resovler
        - get user jwt from orchestrator and create user creds file with provided file path
    - returning the host pubkey and closing client cleanly
*/

use std::{path::PathBuf, time::Duration};

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult, ErrorContext};
use crate::local_cmds::host::types::agent_client::{
    HostClient, HostClientConfig, TypeSpecificArgs,
};
use async_trait::async_trait;

pub const HOST_AUTH_CLIENT_NAME: &str = "Host Auth";
pub const HOST_AUTH_CLIENT_INBOX_PREFIX: &str = "_AUTH_INBOX";

#[derive(Debug)]
pub struct AuthClient {
    pub client: async_nats::Client,
    pub _creds_path: PathBuf,
}

#[async_trait]
impl HostClient for AuthClient {
    type Output = Self;

    async fn start(config: &HostClientConfig) -> HostAgentResult<Self::Output> {
        let client_args = match &config.type_args {
            TypeSpecificArgs::HostAuth(host_auth_args) => host_auth_args,
            _ => {
                return Err(HostAgentError::validation(
                    "Invalid client type for auth client",
                ))
            }
        };

        let user_unique_inbox = format!(
            "{}.{}",
            HOST_AUTH_CLIENT_INBOX_PREFIX, client_args.inbox_sub_prefix
        );

        let auth_guard_client = async_nats::ConnectOptions::new()
            .name(HOST_AUTH_CLIENT_NAME.to_string())
            .custom_inbox_prefix(user_unique_inbox)
            .ping_interval(Duration::from_secs(10))
            .request_timeout(Some(Duration::from_secs(30)))
            .token(client_args.token.clone())
            .credentials_file(&config.nats_creds_path)
            .await
            .map_err(|e| {
                HostAgentError::service_failed(
                    "auth client credentials",
                    &format!("Failed to load credentials file: {}", e),
                )
            })?
            .connect(&client_args.hub_url)
            .await?;

        let server_info = auth_guard_client.server_info();
        log::debug!(
            "User connected to server id #{} on port {}.  Connection State: {:#?}",
            server_info.server_id,
            server_info.port,
            auth_guard_client.connection_state()
        );

        Ok(Self {
            client: auth_guard_client,
            _creds_path: config.nats_creds_path.clone(),
        })
    }

    async fn stop(&self) -> HostAgentResult<()> {
        self.client.drain()
            .await
            .map_err(|e| HostAgentError::service_failed("draining auth client connection", &e.to_string()))?;
        Ok(())
    }
}
