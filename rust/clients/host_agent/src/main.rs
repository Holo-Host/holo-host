mod auth;
mod hostd;
pub mod local_cmds;
pub mod remote_cmds;
mod types;

use clap::Parser;
use dotenv::dotenv;

use local_cmds::host::{errors::HostAgentResult, init_host_d, call_host_info_command};
use local_cmds::support::call_support_command;
use types::{self as app_cli};

#[tokio::main]
async fn main() -> HostAgentResult<()> {
    dotenv().ok();
    env_logger::init();

    let cli = app_cli::Root::parse();
    match cli.scope {
        app_cli::CommandScopes::Daemonize(daemonize_args) => {
            log::info!("Spawning host agent.");
            init_host_d(&daemonize_args).await?;
        }
        app_cli::CommandScopes::Host { command } => {
            call_host_info_command(&command)?;
        }
        app_cli::CommandScopes::Support { command } => {
            call_support_command(&command)?;
        }
        app_cli::CommandScopes::Remote {
            remote_args,
            command,
        } => {
            nats_utils::jetstream_client::tls_skip_verifier::early_in_process_install_crypto_provider();

            remote_cmds::run(remote_args, command).await?;
        }
    }
    Ok(())
}
