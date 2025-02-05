use std::path::PathBuf;

use util_libs::{
    nats_js_client,
    nats_server::{
        self, JetStreamConfig, LeafNodeRemote, LeafServer, LoggingOptions, LEAF_SERVER_CONFIG_PATH,
        LEAF_SERVER_DEFAULT_LISTEN_PORT, LEAF_SERVE_NAME,
    },
};

pub async fn run(user_creds_path: &Option<PathBuf>) {
    let leaf_server_remote_conn_url = nats_server::get_hub_server_url();
    let leaf_client_conn_domain = "127.0.0.1";
    let leaf_client_conn_port = std::env::var("NATS_LISTEN_PORT")
        .map(|var| var.parse().expect("can't parse into number"))
        .unwrap_or_else(|_| LEAF_SERVER_DEFAULT_LISTEN_PORT);

    let nsc_path = nats_js_client::get_nsc_root_path();

    let jetstream_config = JetStreamConfig {
        store_dir: format!("{}/leaf_store", nsc_path),
        max_memory_store: 1024 * 1024 * 1024, // 1 GB
        max_file_store: 1024 * 1024 * 1024,   // 1 GB
    };

    let logging_options = LoggingOptions {
        debug: true, // NB: This logging is a blocking action, only run in non-prod
        trace: true, // NB: This logging is a blocking action, only run in non-prod
        longtime: false,
    };

    // Instantiate the Leaf Server with the user cred file
    let leaf_node_remotes = vec![LeafNodeRemote {
        // sys account user (automated)
        url: leaf_server_remote_conn_url.to_string(),
        credentials_path: user_creds_path.clone(),
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
}
