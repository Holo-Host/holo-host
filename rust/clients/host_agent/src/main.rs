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
pub mod agent_cli;
pub mod host_cmds;
pub mod support_cmds;
use agent_cli::DaemonzeArgs;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
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
        agent_cli::CommandScopes::Daemonize(daemonize_args) => {
            log::info!("Spawning host agent.");
            daemonize(daemonize_args).await?;
        }
        agent_cli::CommandScopes::Host { command } => host_cmds::host_command(command)?,
        agent_cli::CommandScopes::Support { command } => support_cmds::support_command(command)?,
    }

    Ok(())
}

async fn daemonize(args: &DaemonzeArgs) -> Result<(), async_nats::Error> {
    let creds_path_arg_clone = args.nats_leafnode_client_creds_path.clone();
    let host_creds_path = creds_path_arg_clone.unwrap_or_else(|| {
        authentication::utils::get_file_path_buf(
            &util_libs::nats_js_client::get_nats_client_creds("HOLO", "HPOS", "host")
        )
    });
    let host_pubkey: String = match host_creds_path.try_exists() {
        Ok(_p) => {
            // TODO: read creds file and parse out pubkey OR call nsc to read pubkey from file (whichever is cleaner)
            "host_pubkey_placeholder>".to_string()
        },
        Err(_) => {
            log::debug!("About to run the Hosting Agent Initialization Service");
            auth::init_agent::run().await?
        }
    };

    let _ = hostd::gen_leaf_server::run(
        &args.nats_leafnode_client_creds_path,
        &args.store_dir,
        args.hub_url.clone(),
        args.hub_tls_insecure,
    )
    .await;

    let host_workload_client = hostd::workload_manager::run(
        &host_pubkey,
        &args.nats_leafnode_client_creds_path,
        args.nats_connect_timeout_secs,
    )
    .await?;

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close client and drain internal buffer before exiting to make sure all messages are sent
    host_workload_client.close().await?;

    Ok(())
}
