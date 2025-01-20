/*
This client is associated with the:
  - WORKLOAD account
  - hpos user

This client is responsible for subscribing the host agent to workload stream endpoints:
  - installing new workloads
  - removing workloads
  - sending active periodic workload reports
  - sending workload status upon request
*/

mod auth;
mod hostd;
use anyhow::Result;
use auth::utils as local_utils;
use clap::Parser;
use dotenv::dotenv;
pub mod agent_cli;
pub mod host_cmds;
pub mod support_cmds;
use thiserror::Error;

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
        Some(agent_cli::CommandScopes::Daemonize) => {
            log::info!("Spawning host agent.");
            daemonize().await?;
        }
        Some(agent_cli::CommandScopes::Host { command }) => host_cmds::host_command(command)?,
        Some(agent_cli::CommandScopes::Support { command }) => {
            support_cmds::support_command(command)?
        }
        None => {
            log::warn!("No arguments given. Spawning host agent.");
            daemonize().await?;
        }
    }

    Ok(())
}

async fn daemonize() -> Result<(), async_nats::Error> {
    let host_creds_path = local_utils::get_host_credentials_path();
    let host_pubkey: String = match authentication::utils::get_file_path_buf(&host_creds_path).try_exists() {
        Ok(_p) => {
            // TODO: read creds file for pubkey OR call nsc and get pubkey (whichever is cleaner)
            "host_pubkey_placeholder>".to_string()
        },
        Err(_) => {
            log::debug!("About to run the Hosting Agent Initialization Service");
            auth::init_agent::run().await?
        }
    };

    hostd::gen_leaf_server::run(&host_creds_path).await;
    hostd::workload_manager::run(&host_pubkey, &host_creds_path).await?;
    Ok(())
}
