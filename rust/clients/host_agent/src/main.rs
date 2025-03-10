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
pub mod support_cmds;
use agent_cli::DaemonzeArgs;
use anyhow::Result;
use clap::Parser;
use dotenv::dotenv;
use hpos_hal::inventory::HoloInventory;
use thiserror::Error;
use tokio::task::spawn;

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
        agent_cli::CommandScopes::Remote { nats_url, command } => {
            log::info!("Trying to connect to {nats_url}...");

            let nats_client =
                nats_utils::jetstream_client::JsClient::new(nats_utils::types::JsClientBuilder {
                    nats_url: nats_url.to_string(),
                    name: "host-agent-remote-client".to_string(),
                    inbox_prefix: Default::default(),
                    credentials: Default::default(),
                    ping_interval: Some(std::time::Duration::from_secs(10)),
                    request_timeout: Some(std::time::Duration::from_secs(29)),
                    listeners: Default::default(),
                })
                .await
                .map_err(|e| {
                    AgentCliError::AsyncNats(
                        format!("connecting to NATS via {nats_url}: {e:?}").into(),
                    )
                })?;

            match command {
                agent_cli::RemoteCommands::Ping {} => {
                    let check = nats_client.check_connection().await?;

                    log::info!("Connection check result: {check}");
                }
                agent_cli::RemoteCommands::WorkloadsManage {} => {
                    unimplemented!();
                }
            }
        }
    }

    Ok(())
}

async fn daemonize(args: &DaemonzeArgs) -> Result<(), async_nats::Error> {
    // let host_pubkey = auth::init_agent::run().await?;
    let host_inventory = HoloInventory::from_host();
    let host_id = host_inventory.system.machine_id.clone();

    let (bare_client, mut leaf_server) = hostd::gen_leaf_server::run(
        &host_id,
        &args.nats_leafnode_server_name,
        &args.nats_leafnode_client_creds_path,
        &args.store_dir,
        args.hub_url.clone(),
        args.hub_tls_insecure,
        args.nats_connect_timeout_secs,
    )
    .await?;
    // TODO: would it be a good idea to reuse this client in the workload_manager and elsewhere later on?
    bare_client.close().await?;

    let host_client =
        hostd::host_client::run(&host_id, &args.nats_leafnode_client_creds_path).await?;

    // Get Host Agent inventory check duration env var..
    // If none exists, default to 1 hour
    let host_inventory_check_interval_sec =
        &args.host_inventory_check_interval_sec.unwrap_or_else(|| {
            std::env::var("HOST_INVENTORY_CHECK_DURATION")
                .unwrap_or_else(|_| "3600".to_string())
                .parse::<u64>()
                .unwrap_or(3600) // 3600 seconds = 1 hour
        });

    // Get Host Agent inventory storage file path
    // If none exists, default to "/var/lib/holo_inventory.json"
    let inventory_file_path = args.host_inventory_file_path.as_ref().map_or_else(
        || {
            std::env::var("HOST_INVENTORY_FILE_PATH")
                .unwrap_or("/var/lib/holo_inventory.json".to_string())
        },
        |s| s.to_owned(),
    );

    let host_client_inventory_clone = host_client.clone();
    let host_id_inventory_clone = host_id.clone();
    let inventory_interval = host_inventory_check_interval_sec.to_owned();
    spawn(async move {
        if let Err(e) = hostd::inventory::run(
            host_client_inventory_clone,
            &host_id_inventory_clone,
            &inventory_file_path,
            inventory_interval,
            host_inventory,
        )
        .await
        {
            log::error!("Error running host agent workload service. Err={:?}", e)
        };
    });

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
