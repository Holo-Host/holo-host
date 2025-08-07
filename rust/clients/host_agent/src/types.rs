use crate::local_cmds::host::types::agent_cli::{DaemonzeArgs, HostCommands};
use crate::local_cmds::support::types::SupportCommands;
use crate::remote_cmds::types::{RemoteArgs, RemoteCommands};

use clap::{Parser, Subcommand};

/// Module containing all of the Clap Derive structs/definitions that make up the agent's
/// command line. To start the agent daemon (usually from systemd), use `host_agent daemonize`.
#[derive(Parser)]
#[command(
    version,
    about,
    author,
    long_about = "Command line interface for hosting workloads on the Holo Hosting Network"
)]
pub struct Root {
    #[command(subcommand)]
    pub scope: CommandScopes,
}

#[derive(Subcommand, Clone)]
pub enum CommandScopes {
    /// Start the Holo Hosting Agent Daemon.
    Daemonize(DaemonzeArgs),
    /// Commmands for managing this host.
    Host {
        #[command(subcommand)]
        command: HostCommands,
    },
    /// Run Diagnostic Commands.
    Support {
        #[command(subcommand)]
        command: SupportCommands,
    },

    /// Interact with a remote host-agent (via NATS).
    Remote {
        #[clap(flatten)]
        remote_args: RemoteArgs,

        #[command(subcommand)]
        command: RemoteCommands,
    },
}
