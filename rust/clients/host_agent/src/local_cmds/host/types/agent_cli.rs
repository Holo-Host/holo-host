use clap::{Args, Subcommand};
use nats_utils::leaf_server::LEAF_SERVER_DEFAULT_LISTEN_PORT;
use std::path::PathBuf;

/// A set of commands for being able to manage the local host. We may (later) want to gate some
/// of these behind a global `--advanced` option to deter hosters from certain commands, but in the
/// meantime, everything is safe to leave open.
#[derive(Subcommand, Clone)]
pub enum HostCommands {
    /// Display information about the current host model.
    ModelInfo,
}

#[derive(Args, Clone, Debug)]
pub struct DaemonzeArgs {
    #[arg(long, help = "directory to contain the NATS persistence")]
    pub(crate) store_dir: Option<PathBuf>,

    #[arg(help = "path to NATS credentials used for the LeafNode SYS user management")]
    pub(crate) nats_leafnode_client_sys_creds_path: Option<PathBuf>,

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

    #[arg(
        long,
        help = "host agent inventory check interval (in seconds)",
        env = "HOST_INVENTORY_CHECK_DURATION",
        default_value_t = 3600
    )]
    pub(crate) host_inventory_check_interval_sec: u64,

    #[arg(
        long,
        help = "host agent inventory file path",
        env = "HOST_INVENTORY_FILE_PATH",
        default_value = "/var/lib/holo-host-agent/inventory.json"
    )]
    pub(crate) host_inventory_file_path: String,

    #[arg(
        long,
        env = "NATS_LEAF_SERVER_LISTEN_HOST",
        default_value = "127.0.0.1",
        value_parser = |s: &str| url::Host::<String>::parse(s),
    )]
    pub(crate) leaf_server_listen_host: url::Host<String>,

    #[arg(long,
        env = "NATS_LEAF_SERVER_LISTEN_PORT",
        default_value_t = LEAF_SERVER_DEFAULT_LISTEN_PORT,
    )]
    pub(crate) leaf_server_listen_port: u16,
}
