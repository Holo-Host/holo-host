pub mod agent_cli;
pub mod agent_client;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

pub fn validate_args(args: &agent_cli::DaemonzeArgs) -> HostAgentResult<()> {
    // Validate hub URL format for NATS connections (clap ensures it's not empty)
    if !args.hub_url.starts_with("wss://")
        && !args.hub_url.starts_with("tls://")
        && !args.hub_url.starts_with("nats://")
    {
        return Err(HostAgentError::validation(&format!(
            "Hub URL '{}' must start with 'nats://' or 'tls://' for NATS connections. \
             Please provide a valid NATS URL format.",
            args.hub_url
        )));
    }
    Ok(())
}
