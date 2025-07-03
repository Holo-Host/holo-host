pub mod agent_cli;
pub mod agent_d;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

pub fn validate_args(args: &agent_cli::DaemonzeArgs) -> HostAgentResult<()> {
    // Validate hub URL format for NATS connections (clap ensures it's not empty)
    if !args.hub_url.starts_with("nats://") && !args.hub_url.starts_with("tls://") {
        return Err(HostAgentError::validation(&format!(
            "Hub URL '{}' must start with 'nats://' or 'tls://' for NATS connections. \
             Please provide a valid NATS URL format.",
            args.hub_url
        )));
    }
    Ok(())
}
