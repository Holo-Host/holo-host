/*
This client is associated with the:
  - WORKLOAD account
  - host user

This client is responsible for subscribing the host agent to workload stream endpoints:
  - installing new workloads
  - removing workloads
  - sending active periodic workload reports
  - sending workload status upon request
*/

mod auth;
mod hostd;
mod keys;
pub mod agent_cli;
pub mod host_cmds;
pub mod support_cmds;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use thiserror::Error;
use agent_cli::DaemonzeArgs;
use util_libs::nats_js_client;

#[derive(Error, Debug)]
pub enum AgentCliError {
    #[error("Agent Daemon Error")]
    AsyncNats(#[from] async_nats::Error),
    #[error("Command Line Error")]
    CommandError(#[from] std::io::Error),
}

#[tokio::main]
async fn main() -> Result<(), AgentCliError> {
    dotenv().ok();
    env_logger::init();

    let cli = agent_cli::Root::parse();
    match &cli.scope {
        Some(agent_cli::CommandScopes::Daemonize(daemonize_args)) => {
            log::info!("Spawning host agent.");
            daemonize(daemonize_args).await?;
        }
        Some(agent_cli::CommandScopes::Host { command }) => host_cmds::host_command(command)?,
        Some(agent_cli::CommandScopes::Support { command }) => {
            support_cmds::support_command(command)?
        }
        None => {
            log::warn!("No arguments given. Spawning host agent.");
            daemonize(&Default::default()).await?;
        }
    }

    Ok(())
}

async fn daemonize(args: &DaemonzeArgs) -> Result<(), async_nats::Error> {    
    let host_agent_keys = match keys::Keys::try_from_storage(&args.nats_leafnode_client_creds_path, &args.nats_leafnode_client_sys_creds_path)? {
        Some(k) => k,
        None => {
            log::debug!("About to run the Hosting Agent Initialization Service");
            let mut keys = keys::Keys::new()?;
            keys = auth::init::run(keys).await?;
            keys
        }
    };

    hostd::gen_leaf_server::run(&host_agent_keys.get_host_creds_path()).await;
    hostd::workload_manager::run(
        &host_agent_keys.host_pubkey,
        &host_agent_keys.get_host_creds_path(),
        args.nats_connect_timeout_secs,
    )
    .await?;
    Ok(())
}
