pub mod errors;
pub mod types;
pub(crate) mod utils;

use crate::hostd;
use errors::{HostAgentError, HostAgentResult};
use types::{
    self as agent_cli_types,
    agent_cli::{DaemonzeArgs, HostCommands},
};

use tokio::sync::broadcast;
use tokio::task::JoinSet;

use hpos_hal::inventory::HoloInventory;

pub fn call_host_info_command(command: &HostCommands) -> HostAgentResult<()> {
    // TODO: Fill these in under a separate set of commits to keep PRs simple.
    match command {
        HostCommands::ModelInfo => {
            let inventory = HoloInventory::from_host();
            match inventory.platform {
                Some(p) => {
                    println!("{}", p);
                    Ok(())
                }
                None => Err(HostAgentError::system_info_unavailable(
                    "platform information",
                )),
            }
        }
    }
}

pub async fn init_host_d(args: &DaemonzeArgs) -> HostAgentResult<()> {
    agent_cli_types::validate_args(args)?;

    // Setup service shutdown mechanism
    let (shutdown_tx, _) = broadcast::channel(1);
    let mut services = JoinSet::new();

    // Load host inventory and device ID
    let host_inventory = HoloInventory::from_host();
    let device_id = host_inventory.system.machine_id;
    if device_id.is_empty() {
        return Err(HostAgentError::validation(
            "Host device ID is empty. This indicates a problem with the system inventory or machine ID generation. \
             Please check that the system is properly configured and the machine ID is available."
        ));
    }

    // TODO: Run agent auth

    // Once authenticated, start leaf server
    let leaf_server = hostd::leaf_server_generator::run(
        &device_id,
        &args.nats_leafnode_server_name,
        &None,
        &args.store_dir,
        &args.hub_url,
        args.hub_tls_insecure,
        Some(args.hub_jetstream_domain.clone()),
        args.nats_connect_timeout_secs,
        &args.leaf_server_listen_host,
        args.leaf_server_listen_port,
        shutdown_tx.subscribe(),
    )
    .await?;

    // Get leaf server address before spawning services
    let leaf_server_addr: nats_utils::types::DeServerAddr = leaf_server.server_addr()?.into();

    // Spawn the host agent services (inventory and workload)
    // and add them to the JoinSet for shutdown handling
    services.spawn({
        let args = args.clone();
        let leaf_server_addr = leaf_server_addr.clone();
        let shutdown_tx = shutdown_tx.clone();
        async move {
            hostd::services::run(
                &device_id,
                &leaf_server_addr,
                &args,
                shutdown_tx.subscribe(),
            )
            .await
        }
    });

    log::info!(
        "Host Agent is connected to the Leaf Nats Server at {:?} and starting {} services.",
        leaf_server_addr,
        services.len()
    );

    // Wait for either a shutdown signal or service failure.
    // NB: This keeps the host agent running until either of these conditions is met.
    let shutdown_reason = tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("Shutdown signal (Ctrl+C) received");
            "shutdown_signal"
        }
        result = services.join_next() => {
            if let Some(Err(e)) = result {
                log::error!("Service failed: {}", e);
                "service_failure"
            } else {
                log::warn!("Unexpected service closure");
                "unexpected_closure"
            }
        }
    };

    utils::graceful_shutdown(shutdown_reason, shutdown_tx, services, leaf_server).await
}
