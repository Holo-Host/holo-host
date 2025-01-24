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
use agent_cli::DaemonzeArgs;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
pub mod agent_cli;
pub mod gen_leaf_server;
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
    // let (host_pubkey, host_creds_path) = auth::initializer::run().await?;
    let _ = gen_leaf_server::run(
        &args.nats_leafnode_client_creds_path,
        &args.store_dir,
        args.hub_url.clone(),
        args.hub_tls_insecure,
    )
    .await;

    let _ = workload_manager::run(
        "host_id_placeholder>",
        &args.nats_leafnode_client_creds_path,
        args.nats_connect_timeout_secs,
    )
    .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    Ok(())
}
