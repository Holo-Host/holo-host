use clap::Subcommand;
use netdiag::IPVersion;

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
