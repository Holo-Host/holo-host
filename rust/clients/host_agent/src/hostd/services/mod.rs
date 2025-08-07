pub mod inventory;
mod utils;
pub mod workload;

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinSet;

use crate::hostd::client::HostAgentClient;
use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};
use crate::local_cmds::host::types::agent_cli::DaemonzeArgs;
use crate::local_cmds::host::types::agent_client::{
    ClientType, HostClient, HostClientConfig, HostDArgs,
};

use nats_utils::types::DeServerAddr;

pub async fn run(
    device_id: &str,
    nats_url: &DeServerAddr,
    args: &DaemonzeArgs,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> HostAgentResult<()> {
    let mut hostd_client_tasks: JoinSet<std::result::Result<_, _>> = JoinSet::new();

    // Create Host Client Config and start main Host Agent Client
    let host_client_config = HostClientConfig::new(
        device_id,
        ClientType::HostAgent(HostDArgs {
            nats_url: nats_url.clone(),
        }),
    )?;
    let hostd_client = HostAgentClient::start(&host_client_config).await?.client;
    let hostd_client_arc = Arc::new(RwLock::new(hostd_client.clone()));

    // Spawn inventory service
    let host_inventory_file_path = &args.host_inventory_file_path;
    let inventory_svc_interval = args.host_inventory_check_interval_sec;
    let (shutdown_tx, _) = broadcast::channel(1);
    let inventory_shutdown_rx = shutdown_tx.subscribe();

    hostd_client_tasks.spawn({
        let hostd_client_arc = Arc::clone(&hostd_client_arc);
        let device_id = device_id.to_string();
        let host_inventory_file_path = host_inventory_file_path.clone();
        async move {
            log::info!("Starting inventory service...");
            inventory::run(
                &mut (*hostd_client_arc.write().await),
                &device_id,
                &host_inventory_file_path,
                inventory_svc_interval,
                inventory_shutdown_rx,
            )
            .await
        }
    });

    // Spawn workload service
    let hub_jetstream_domain = args.hub_jetstream_domain.clone();
    let workload_shutdown_rx = shutdown_tx.subscribe();
    hostd_client_tasks.spawn({
        let hostd_client_arc = Arc::clone(&hostd_client_arc);
        let device_id = device_id.to_string();
        async move {
            log::info!("Starting workload service...");
            workload::run(
                hostd_client_arc,
                &device_id,
                &hub_jetstream_domain,
                workload_shutdown_rx,
            )
            .await
        }
    });

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                log::info!("Hostd agent client services shutting down");
                // Send shutdown signal to all services
                let _ = shutdown_tx.send(());
                break;
            }
            result = hostd_client_tasks.join_next() => {
                if let Some(Err(e)) = result {
                    log::error!("Service task failed: {:?}", e);
                    return Err(HostAgentError::service_failed("service task", &format!("Service task failed: {:?}", e)));
                }
            }
        }
    }

    // Shutdown gracefully
    hostd_client_tasks.shutdown().await;
    while let Some(result) = hostd_client_tasks.join_next().await {
        match result {
            Ok(Ok(_)) => log::debug!("Client exited successfully"),
            Ok(Err(e)) => log::warn!(
                "Hostd client service (eg: workload or inventory) exited with error: {}",
                e
            ),
            Err(e) => log::error!("Client task join error: {}", e),
        }
    }

    log::info!("Hostd client stopped");
    let host_agent_client = HostAgentClient {
        client: hostd_client,
    };
    host_agent_client.stop().await?;

    Ok(())
}
