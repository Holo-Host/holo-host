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

mod workload_manager;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
pub mod agent_cli;
pub mod gen_leaf_server;
pub mod host_cmds;
pub mod support_cmds;
use thiserror::Error;
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
    // let (host_pubkey, host_creds_path) = auth::initializer::run().await?;
    let host_creds_path = nats_js_client::get_nats_client_creds("HOLO", "HPOS", "hpos");
    let host_pubkey = "host_id_placeholder>";
    gen_leaf_server::run(&host_creds_path).await;
    workload_manager::run(host_pubkey, &host_creds_path).await?;
    Ok(())
}
