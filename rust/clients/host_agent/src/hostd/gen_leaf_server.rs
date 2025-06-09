use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use nats_utils::{
    jetstream_client,
    leaf_server::{
        JetStreamConfig, LeafNodeRemote, LeafNodeRemoteTlsConfig, LeafServer, LoggingOptions,
        LEAF_SERVER_CONFIG_PATH,
    },
    types::{DeServerAddr, JsClientBuilder, NatsRemoteArgs},
};
use tempfile::tempdir;
use url::Host;

#[allow(clippy::too_many_arguments)]
pub async fn run(
    host_id: &str,
    maybe_server_name: &Option<String>,
    user_creds_path: &Option<PathBuf>,
    maybe_store_dir: &Option<PathBuf>,
    hub_url: String,
    hub_tls_insecure: bool,
    nats_connect_timeout_secs: u64,
    leaf_server_listen_host: Host<String>,
    leaf_server_listen_port: u16,
) -> anyhow::Result<(jetstream_client::JsClient, LeafServer)> {
    let (
        store_dir,
        _, // need to prevent the tempdir from dropping
    ) = if let Some(store_dir) = maybe_store_dir {
        std::fs::create_dir_all(store_dir).context("creating {store_dir:?}")?;
        (store_dir.clone(), None)
    } else {
        let maybe_tempfile = tempdir()?;
        (maybe_tempfile.path().to_owned(), Some(tempdir))
    };

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
        url: hub_url,
        // sys account user (automated)
        credentials: user_creds_path.clone(),
        tls: LeafNodeRemoteTlsConfig {
            insecure: hub_tls_insecure,
            ..Default::default()
        },
    }];

    // The hub needs a unique name for each server to distinguish the leaf node connection
    let server_name = if let Some(server_name) = maybe_server_name {
        server_name.clone()
    } else {
        host_id.to_string()
    };

    log::info!("Spawning Leaf Server");
    // Create a new Leaf Server instance
    let mut leaf_server = LeafServer::new(
        Some(&server_name),
        LEAF_SERVER_CONFIG_PATH,
        leaf_server_listen_host,
        leaf_server_listen_port,
        Some(3145728), // 3MiB in bytes (temp solution - matches holo-nats server configuration)
        jetstream_config,
        logging_options,
        leaf_node_remotes,
    );
    leaf_server
        .run()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))
        .context("Failed to spawn the Leaf Server in a separate thread")?;

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
                        nats_url: DeServerAddr(nats_url.clone()),
                        ..Default::default()
                    },
                    name:HOST_AGENT_CLIENT_NAME.to_string(),
                    ping_interval:Some(Duration::from_secs(10)),
                    request_timeout:Some(Duration::from_secs(29)),

                    ..Default::default()
                })
                .await
                .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url:?}: {e:?}"));

                match host_workload_client {
                    Ok(client) => break client,
                    Err(e) => {
                        let duration = tokio::time::Duration::from_millis(100);
                        log::warn!("{e:?}, retrying in {duration:?}");
                        tokio::time::sleep(duration).await;
                    }
                }
            }} => client,
        _ = {
            log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
         } => {
            anyhow::bail!("timed out waiting for NATS on {nats_url:?}");
        }
    };

    Ok((nats_client, leaf_server))
}
