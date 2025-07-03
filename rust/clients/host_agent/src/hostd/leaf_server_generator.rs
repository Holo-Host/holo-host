use std::{path::PathBuf, time::Duration};
use tempfile::tempdir;
use tokio::sync::broadcast;
use url::Host;

use nats_utils::{
    jetstream_client,
    leaf_server::{
        JetStreamConfig, LeafNodeRemote, LeafNodeRemoteTlsConfig, LeafServer, LoggingOptions,
    },
    types::{JsClientBuilder, NatsRemoteArgs},
};

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

#[allow(clippy::too_many_arguments)]
pub async fn run(
    host_id: &str,
    maybe_server_name: &Option<String>,
    user_creds_path: &Option<PathBuf>,
    maybe_store_dir: &Option<PathBuf>,
    hub_url: &str,
    hub_tls_insecure: bool,
    nats_connect_timeout_secs: u64,
    leaf_server_listen_host: &Host<String>,
    leaf_server_listen_port: u16,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> HostAgentResult<LeafServer> {
    let (
        store_dir,
        _, // need to prevent the tempdir from dropping
    ) = if let Some(store_dir) = maybe_store_dir {
        std::fs::create_dir_all(store_dir)?;
        (store_dir.clone(), None)
    } else {
        let maybe_tempfile = tempdir()?;
        (maybe_tempfile.path().to_owned(), Some(tempdir))
    };

    // Create the config file path within the store directory
    let config_path = store_dir.join("leaf_server.conf");
    let config_path_str = config_path.to_str().ok_or_else(|| {
        HostAgentError::service_failed(
            "leaf server config path",
            &format!(
                "Failed to convert config path {:?} to UTF-8 string",
                config_path
            ),
        )
    })?;

    let jetstream_config = JetStreamConfig {
        store_dir,
        // TODO: make this configurable
        max_memory_store: 1024 * 1024 * 1024, // 1 GB
        // TODO: make this configurable
        max_file_store: 1024 * 1024 * 1024, // 1 GB
    };

    let logging_options = LoggingOptions {
        // TODO: make this configurable
        debug: true, // NB: This logging is a blocking action, only run in non-prod
        // TODO: make this configurable
        trace: false, // NB: This logging is a blocking action, only run in non-prod
        logtime: false,
    };

    // Instantiate the Leaf Server with the user cred file
    let leaf_node_remotes = vec![LeafNodeRemote {
        url: hub_url.to_string(),
        // sys account user (automated)
        credentials: user_creds_path.clone(),
        tls: LeafNodeRemoteTlsConfig {
            insecure: hub_tls_insecure,
            ..Default::default()
        },
    }];

    // The hub needs a unique name for each server to distinguish the leaf node connection
    let leaf_server_name = maybe_server_name.as_deref().unwrap_or(host_id);

    log::info!("Spawning Leaf Server");
    // Create a new Leaf Server instance
    let mut leaf_server = LeafServer::new(
        Some(leaf_server_name),
        config_path_str,
        leaf_server_listen_host.clone(),
        leaf_server_listen_port,
        Some(3145728), // 3MiB in bytes (temp solution - matches holo-nats server configuration)
        jetstream_config,
        logging_options,
        leaf_node_remotes,
    );

    // Start the leaf server and handle shutdown during startup
    let leaf_server_result = tokio::select! {
        result = leaf_server.run() => result,
        _ = shutdown_rx.recv() => {
            log::info!("Shutdown signal received during leaf server startup");
            return Err(HostAgentError::service_failed(
                "leaf server startup",
                "Shutdown signal received during leaf server startup"
            ));
        }
    };

    leaf_server_result?;

    // Spin up Nats Client
    // Nats takes a moment to become responsive, so we try to connecti in a loop for a few seconds.
    // in case of a connection loss to Nats this client is self-recovering.
    let nats_url = leaf_server.server_addr()?;
    log::info!("nats_url : {nats_url:?}");

    const HOST_AGENT_CLIENT_NAME: &str = "Host Agent Bare";

    let nats_client = tokio::select! {
        client = async {loop {
            let host_workload_client = jetstream_client::JsClient::new(JsClientBuilder {
                nats_remote_args: NatsRemoteArgs {
                    nats_url: nats_url.clone().into(),
                    ..Default::default()
                },
                name:HOST_AGENT_CLIENT_NAME.to_string(),
                ping_interval:Some(Duration::from_secs(10)),
                request_timeout:Some(Duration::from_secs(29)),

                ..Default::default()
            })
            .await
            .map_err(HostAgentError::from);
            match host_workload_client {
                Ok(client) => break client,
                Err(e) => {
                    let duration = tokio::time::Duration::from_millis(100);
                    log::warn!("{e:?}, retrying in {duration:?}");
                    tokio::time::sleep(duration).await;
                }
            }}} => client,
        _ = {
            log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
         } => {
            log::error!("Timed out waiting for NATS on {nats_url:?}");
            // Ensure leaf server is closed before bailing out
            if let Err(e) = leaf_server.close().await {
                log::warn!("Failed to close leaf server after NATS timeout: {}", e);
            }
            return Err(HostAgentError::service_failed(
                "NATS connection timeout",
                &format!("timed out waiting for NATS on {nats_url:?}")
            ));
        }
        _ = shutdown_rx.recv() => {
            log::info!("Shutdown signal received during NATS client setup");
            // Ensure leaf server is closed before returning error
            if let Err(e) = leaf_server.close().await {
                log::warn!("Failed to close leaf server after shutdown signal: {}", e);
            }
            return Err(HostAgentError::service_failed(
                "NATS client setup",
                "Shutdown signal received during NATS client setup"
            ));
        }
    };

    // Close the NATS client before returning the leaf server
    // TODO: Look into why this is needed and remove..
    if let Err(e) = nats_client.close().await {
        log::warn!("Failed to close NATS client: {}", e);
    }

    // TODO: why does NATS need some time here?
    // ATTN: without this time the inventory isn't always sent..
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(5)) => {
            log::debug!("Completed 5-second wait period");
        }
        _ = shutdown_rx.recv() => {
            log::info!("Shutdown signal received during wait period");
            // Ensure leaf server is closed before returning error
            if let Err(e) = leaf_server.close().await {
                log::warn!("Failed to close leaf server after shutdown signal: {}", e);
            }
            return Err(HostAgentError::service_failed(
                "leaf server wait period",
                "Shutdown signal received during wait period"
            ));
        }
    }

    Ok(leaf_server)
}
