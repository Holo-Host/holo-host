use std::path::PathBuf;

use anyhow::Context;
use tempfile::tempdir;
use util_libs::nats_server::{
    JetStreamConfig, LeafNodeRemote, LeafNodeRemoteTlsConfig, LeafServer, LoggingOptions,
    LEAF_SERVER_CONFIG_PATH, LEAF_SERVER_DEFAULT_LISTEN_PORT, LEAF_SERVE_NAME,
};

pub async fn run(
    user_creds_path: &Option<PathBuf>,
    maybe_store_dir: &Option<PathBuf>,
    hub_url: String,
    hub_tls_insecure: bool,
) -> anyhow::Result<()> {
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

    // Create a new Leaf Server instance
    let leaf_server = LeafServer::new(
        LEAF_SERVE_NAME,
        LEAF_SERVER_CONFIG_PATH,
        leaf_client_conn_domain,
        leaf_client_conn_port,
        jetstream_config,
        logging_options,
        leaf_node_remotes,
    );

    log::info!("Spawning Leaf Server");
    let leaf_server_clone = leaf_server.clone();
    // Start the Leaf Server in a separate thread
    let leaf_server_task = tokio::spawn(async move {
        leaf_server_clone
            .run()
            .await
            .expect("Failed to run Leaf Server");
    });

    // Await server task termination
    let _ = leaf_server_task.await;

    Ok(())
}
