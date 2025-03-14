/*
This client is associated with the:
  - HPOS account
  - host user

This client is responsible for subscribing the host agent to workload stream endpoints:
  - installing new workloads
  - removing workloads
  - sending active periodic workload reports
  - sending workload status upon request
*/

pub mod agent_cli;
pub mod host_cmds;
mod hostd;
mod remote_cmds;
pub mod support_cmds;

use std::time::Duration;

use agent_cli::DaemonzeArgs;
use clap::Parser;
use dotenv::dotenv;
use hpos_hal::inventory::HoloInventory;
use thiserror::Error;
use tokio::task::spawn;

pub const HOST_PUBKEY_PLACEHOLDER: &str = "host_pubkey_placeholder";

#[derive(Error, Debug)]
pub enum AgentCliError {
    #[error("Agent Daemon Error")]
    AsyncNats(#[from] async_nats::Error),
    #[error("Command Line Error")]
    CommandError(#[from] std::io::Error),
    #[error("Invalid Arguments: {0}")]
    InvalidArguments(String),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    let cli = agent_cli::Root::parse();
    match cli.scope {
        agent_cli::CommandScopes::Daemonize(daemonize_args) => {
            log::info!("Spawning host agent.");
            daemonize(&daemonize_args).await?;
        }
        agent_cli::CommandScopes::Host { command } => host_cmds::host_command(&command)?,
        agent_cli::CommandScopes::Support { command } => support_cmds::support_command(&command)?,
        agent_cli::CommandScopes::Remote { nats_url, command } => {
            remote_cmds::run(nats_url, command).await?
        }
    }

    Ok(())
}

async fn daemonize(args: &DaemonzeArgs) -> anyhow::Result<()> {
    // let host_pubkey = auth::init_agent::run().await?;
    let host_inventory = HoloInventory::from_host();
    let host_id = host_inventory.system.machine_id.clone();

    if host_id.is_empty() {
        anyhow::bail!("host_id is empty")
    }

    let (bare_client, mut leaf_server) = hostd::gen_leaf_server::run(
        &host_id,
        &args.nats_leafnode_server_name,
        &args.nats_leafnode_client_creds_path,
        &args.store_dir,
        args.hub_url.clone(),
        args.hub_tls_insecure,
        args.nats_connect_timeout_secs,
        &args.nats_url,
    )
    .await?;
    // TODO: would it be a good idea to reuse this client in the workload_manager and elsewhere later on?

    bare_client.close().await.map_err(AgentCliError::from)?;

    // TODO: why does NATS need some time here? without this the inventory isn't always sent
    tokio::time::sleep(Duration::from_secs(5)).await;

    let host_client = hostd::host_client::run(
        &host_id,
        &args.nats_leafnode_client_creds_path,
        &args.nats_url,
    )
    .await?;

    if !args.host_inventory_disable {
        let host_inventory_file_path = args.host_inventory_file_path.clone();
        let host_client_inventory = host_client.clone();
        let host_id_inventory = host_id.clone();
        let inventory_interval = args.host_inventory_check_interval_sec;

        spawn(async move {
            if let Err(e) = hostd::inventory::run(
                host_client_inventory,
                &host_id_inventory,
                &host_inventory_file_path,
                inventory_interval,
                host_inventory,
            )
            .await
            {
                log::error!("Error running host agent workload service. Err={:?}", e)
            };
        });
    }

    let host_client_workload_clone = host_client.clone();
    spawn(async move {
        if let Err(e) = hostd::workload::run(host_client_workload_clone, &host_id).await {
            log::error!("Error running host agent workload service. Err={:?}", e)
        };
    });

    // Only exit program when explicitly requested
    tokio::signal::ctrl_c().await?;

    // Close host client connection and drain internal buffer before exiting to make sure all messages are sent
    // NB: Calling drain/close on any one of the Client instances will close the underlying connection.
    // This affects all instances that share the same connection (including clones) because they are all references to the same resource.
    let _ = host_client.close().await;
    let _ = leaf_server.close().await;
    Ok(())
}
