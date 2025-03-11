use clap::{Args, Parser, Subcommand};
use netdiag::IPVersion;
use std::path::PathBuf;
use url::Url;

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
        /// Url for the NATS connection. Can contain credentials.
        #[clap(long, env = "HOST_AGENT_NATS_URL")]
        nats_url: Url,

        #[command(subcommand)]
        command: RemoteCommands,
    },
}

#[derive(Args, Clone, Debug)]
pub struct DaemonzeArgs {
    #[arg(long, help = "directory to contain the NATS persistence")]
    pub(crate) store_dir: Option<PathBuf>,

    #[arg(
        long,
        help = "path to NATS credentials used for the LeafNode client connection"
    )]
    pub(crate) nats_leafnode_client_creds_path: Option<PathBuf>,

    #[arg(
        long,
        help = "server_name used in the LeafNode NATS server. must be unique on the hub."
    )]
    pub(crate) nats_leafnode_server_name: Option<String>,

    #[arg(long, help = "connection URL to the hub")]
    pub(crate) hub_url: String,

    #[arg(
        long,
        help = "whether to tolerate unknown remote TLS certificates for the connection to the hub"
    )]
    pub(crate) hub_tls_insecure: bool,

    #[arg(
        long,
        help = "try to connect to the (internally spawned) Nats instance for the given duration in seconds before giving up",
        default_value = "30"
    )]
    pub(crate) nats_connect_timeout_secs: u64,

    #[arg(long, help = "host agent inventory check interval (in seconds)")]
    pub(crate) host_inventory_check_interval_sec: Option<u64>,

    #[arg(long, help = "host agent inventory file path")]
    pub(crate) host_inventory_file_path: Option<String>,

    #[arg(
        long,
        short,
        help = "disable host agent inventory functionality",
        default_value_t = false
    )]
    pub(crate) host_inventory_disable: bool,
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
    NetTest {
        #[arg(long, default_value("1.1.1.1:53"))]
        nameserver: String,
        // Once we have a URL we can use, make it the default.
        #[arg(long, default_value("holo.host"))]
        hostname: String,
        #[arg(long, default_value("true"))]
        use_tls: bool,
        #[arg(long, default_value("443"))]
        port: u16,
        // As with the hostname, we should change this default once we have something public.
        #[arg(long, default_value("/status"))]
        http_path: String,
        #[arg(long, default_value("ipv4"))]
        ip_version: IPVersion,
    },
    /// Enable or disable a tunnel for support to control this host remotely.
    SupportTunnel {
        #[arg(long)]
        enable: bool,
    },
}

/// A set of commands for remotely interacting with a running host-agent instance, by exchanging NATS messages.
#[derive(Subcommand, Clone)]
pub enum RemoteCommands {
    /// Status
    Ping {},

    /// Manage workloads.
    Workload {
        #[arg(long)]
        operation: String,

        #[arg(long)]
        data: String,
    },
}
