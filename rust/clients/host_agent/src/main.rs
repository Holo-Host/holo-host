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
mod auth;
pub mod host_cmds;
mod hostd;
mod keys;
mod remote_cmds;
pub mod support_cmds;

use agent_cli::DaemonzeArgs;
use clap::Parser;
use dotenv::dotenv;
// use nats_utils::jetstream_client::get_nats_url;
use hpos_hal::inventory::HoloInventory;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::task::spawn;

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
        agent_cli::CommandScopes::Remote {
            remote_args,
            command,
        } => {
            nats_utils::jetstream_client::tls_skip_verifier::early_in_process_install_crypto_provider();

            remote_cmds::run(remote_args, command).await?
        }
    }

    Ok(())
}

async fn daemonize(args: &DaemonzeArgs) -> anyhow::Result<()> {
    let host_inventory = HoloInventory::from_host();
    let device_id = host_inventory.system.machine_id.clone();

    if device_id.is_empty() {
        anyhow::bail!(
            "Host device ID is empty. This indicates a problem with the system inventory or machine ID generation. \
             Please check that the system is properly configured and the machine ID is available."
        )
    }

    let mut host_agent_keys = keys::Keys::try_from_storage(
        &args.nats_leafnode_client_creds_path,
        &args.nats_leafnode_client_sys_creds_path,
    )
    .or_else(|storage_err| {
        log::warn!("Failed to load keys from storage: {}", storage_err);
        log::info!("Attempting to create new keys...");
        keys::Keys::new().map_err(|e| {
            log::error!("Failed to create new keys: {}", e);
            anyhow::anyhow!(
                "Failed to initialize host agent keys. Storage err={}. Key gen err={}",
                storage_err,
                e
            )
        })
    })?;

    // If user cred file is for the auth_guard user, run loop to authenticate host & hoster.
    // This loop will run the authentication handshake with the orchestrator auth service.
    // If successful, it will store the newly auth'd keys and exit out of the loop
    // If unsuccessful, it will reattempt authentication every 24hrs (up to 5 times) and report
    // the unauth'd inventory call, allowing time for the auth to be investigated/resolved.
    if let keys::AuthCredType::Guard(_) = host_agent_keys.creds {
        log::info!("Starting authentication validation loop for device: {}", device_id);
        host_agent_keys = auth::run_validation_loop(device_id.clone(), host_agent_keys, &args.hub_url)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to authenticate host agent with orchestrator at {}: {}",
                    args.hub_url,
                    e
                )
            })?;
        log::info!("Successfully completed authentication validation loop");
    }

    log::trace!(
        "Host Agent Keys after successful authentication: {:#?}",
        host_agent_keys
    );

    // Once authenticated, start leaf server and run workload api calls.
    let (bare_client, mut leaf_server) = hostd::gen_leaf_server::run(
        &device_id,
        &args.nats_leafnode_server_name,
        &host_agent_keys.get_host_creds_path(),
        &args.store_dir,
        args.hub_url.clone(),
        args.hub_tls_insecure,
        args.nats_connect_timeout_secs,
        args.leaf_server_listen_host.clone(),
        args.leaf_server_listen_port,
    )
    .await?;

    // TODO: would it be a good idea to reuse this client in the workload_manager and elsewhere later on?
    bare_client.close().await.map_err(AgentCliError::from)?;

    // TODO: why does NATS need some time here?
    // ATTN: without this time the inventory isn't always sent.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let host_client = hostd::host_client::run(
        &device_id,
        &host_agent_keys.get_host_creds_path(),
        &leaf_server.server_addr()?,
    )
    .await?;

    {
        let host_inventory_file_path = args.host_inventory_file_path.clone();
        let inventory_svc_host_client = host_client.clone();
        let inventory_svc_device_id = device_id.clone();
        let inventory_svc_interval = args.host_inventory_check_interval_sec;

        spawn(async move {
            hostd::inventory::run(
                inventory_svc_host_client,
                &inventory_svc_device_id,
                &host_inventory_file_path,
                inventory_svc_interval,
            )
            .await;
        });
    }

    let workload_svc_host_client = Arc::new(tokio::sync::RwLock::new(host_client.clone()));
    spawn(async move {
        if let Err(e) = hostd::workload::run(workload_svc_host_client, &device_id).await {
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
