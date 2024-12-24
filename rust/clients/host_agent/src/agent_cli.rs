/// MOdule containing all of the Clap Derive structs/definitions that make up the agent's
/// command line. To start the agent daemon (usually from systemd), use `host_agent daemonize`.
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    version,
    about,
    author,
    long_about = "Command line interface for hosting workloads on the Holo Hosting Network"
)]
pub struct Root {
    #[command(subcommand)]
    pub scope: Option<CommandScopes>,
}

#[derive(Subcommand, Clone)]
pub enum CommandScopes {
    /// Start the Holo Hosting Agent Daemon.
    Daemonize,
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
}

/// A set of commands for being able to manage the local host. We may (later) want to gate some
/// of these behind a global `--advanced` option to deter hosters from certain commands, but in the
/// meantime, everything is safe to leave open.
#[derive(Subcommand, Clone)]
pub enum HostCommands {
    /// Display information about the current host model.
    ModelInfo,
}

// Include a set of useful diagnostic commands to aid support. We should work very hard to keep
// this to a small number of specifically useful commands (ie, no more than half a dozen) that give
// results specifically useful to support. We don't want this to become a free-for-all for adding a
// bunch of magical incantations. Each command should be harmless to run and obvious what it does.
#[derive(Subcommand, Clone)]
pub enum SupportCommands {
    /// Run some basic network connectivity diagnostics.
    NetTest,
}
