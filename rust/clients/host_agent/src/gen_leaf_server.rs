use util_libs::nats_server::{JetStreamConfig, LeafNodeRemote, LeafServer, LoggingOptions};

const LEAF_SERVE_NAME: &str = "test_leaf_server";
const LEAF_SERVER_CONFIG_PATH: &str = "test_leaf_server";

pub async fn run(user_creds_path: &str) {
    let leaf_server_remote_conn_url = "nats://127.0.0.1:7422";
    let leaf_client_conn_domain = "127.0.0.1";
    let leaf_client_conn_port = 4111;

    let nsc_path =
        std::env::var("NSC_PATH").unwrap_or_else(|_| ".local/share/nats/nsc".to_string());

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
        credentials_path: user_creds_path.to_string(),
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
