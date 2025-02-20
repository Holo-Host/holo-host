use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use tempfile::tempdir;
use util_libs::nats::{
    jetstream_client,
    leaf_server::{
        JetStreamConfig, LeafNodeRemote, LeafNodeRemoteTlsConfig, LeafServer, LoggingOptions,
        LEAF_SERVER_CONFIG_PATH, LEAF_SERVER_DEFAULT_LISTEN_PORT,
    },
    types::JsClientBuilder,
};

pub async fn run(
    maybe_server_name: &Option<String>,
    user_creds_path: &Option<PathBuf>,
    maybe_store_dir: &Option<PathBuf>,
    hub_url: String,
    hub_tls_insecure: bool,
    nats_connect_timeout_secs: u64,
) -> anyhow::Result<jetstream_client::JsClient> {
    let leaf_client_conn_domain = "127.0.0.1";
    let leaf_client_conn_port = std::env::var("NATS_LISTEN_PORT")
        .map(|var| var.parse().expect("can't parse into number"))
        .unwrap_or_else(|_| LEAF_SERVER_DEFAULT_LISTEN_PORT);

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
        longtime: false,
    };

    // Instantiate the Leaf Server with the user cred file
    let leaf_node_remotes = vec![LeafNodeRemote {
        // sys account user (automated)
        url: hub_url,
        credentials: user_creds_path.clone(),
        tls: LeafNodeRemoteTlsConfig {
            insecure: hub_tls_insecure,
            ..Default::default()
        },
    }];

    let server_name = if let Some(server_name) = maybe_server_name {
        server_name.clone()
    } else {
        // the hub needs a unique name for each server to distinguish the leaf node connection
        machineid_rs::IdBuilder::new(machineid_rs::Encryption::SHA256)
            .add_component(machineid_rs::HWIDComponent::SystemID)
            .build("host-agent")?
    };

    // Create a new Leaf Server instance
    let leaf_server = LeafServer::new(
        Some(&server_name),
        LEAF_SERVER_CONFIG_PATH,
        leaf_client_conn_domain,
        leaf_client_conn_port,
        jetstream_config,
        logging_options,
        leaf_node_remotes,
    );

    log::info!("Spawning Leaf Server");
    tokio::spawn(async move {
        if let Err(e) = leaf_server.run().await {
            anyhow::bail!("failed to run Leaf Server: {e}")
        };

        Ok(())
    })
    .await
    .context("failed to spawn the Leaf Server in a separate thread")??;

    // Spin up Nats Client
    // Nats takes a moment to become responsive, so we try to connecti in a loop for a few seconds.
    // TODO: how do we recover from a connection loss to Nats in case it crashes or something else?
    let nats_url = jetstream_client::get_nats_url();
    log::info!("nats_url : {}", nats_url);

    const HOST_AGENT_CLIENT_NAME: &str = "Host Agent Bare";

    let nats_client = tokio::select! {
        client = async {loop {
                let host_workload_client = jetstream_client::JsClient::new(JsClientBuilder {
                    nats_url:nats_url.clone(),
                    name:HOST_AGENT_CLIENT_NAME.to_string(),
                    ping_interval:Some(Duration::from_secs(10)),
                    request_timeout:Some(Duration::from_secs(29)),

                    inbox_prefix: Default::default(),
                    service_params:Default::default(),
                    listeners: Default::default(),
                    credentials_path: Default::default()
                })
                .await
                .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"));

                match host_workload_client {
                    Ok(client) => break client,
                    Err(e) => {
                        let duration = tokio::time::Duration::from_millis(100);
                        log::warn!("{}, retrying in {duration:?}", e);
                        tokio::time::sleep(duration).await;
                    }
                }
            }} => client,
        _ = {
            log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
         } => {
            anyhow::bail!("timed out waiting for NATS on {nats_url}");
        }
    };

    Ok(nats_client)
}
