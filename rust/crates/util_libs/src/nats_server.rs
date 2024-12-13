/* --------
    This file contains the configuration required to set up a NATS Leaf Server with the "Operator JWT" auth approach.
    NB: This setup expects the `nats-server` binary to be locally installed and accessible.
-------- */
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::process::{Child, Command};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct JetStreamConfig {
    store_dir: String,
    max_memory_store: u64,
    max_file_store: u64,
}

#[derive(Debug, Clone)]
pub struct LoggingOptions {
    debug: bool,
    trace: bool,
    longtime: bool,
}

#[derive(Debug, Clone)]
pub struct LeafNodeRemote {
    url: String,
    credentials_path: String,
    account_key: String,
}

#[derive(Debug, Clone)]
pub struct LeafServer {
    name: String,
    host: String,
    port: u16,
    jetstream_config: JetStreamConfig,
    operator_path: String,
    logging: LoggingOptions,
    leaf_nodes_remote: Vec<LeafNodeRemote>,
    resolver_path: String,
    server_handle: Arc<Mutex<Option<Child>>>,
}

impl LeafServer {
    // Instantiate a new leaf server
    pub fn new(
        server_name: &str,
        host: &str,
        port: u16,
        jetstream_config: JetStreamConfig,
        operator_path: &str,
        logging: LoggingOptions,
        leaf_nodes_remote: Vec<LeafNodeRemote>,
        resolver_path: &str,
    ) -> Self {
        Self {
            name: server_name.to_string(),
            host: host.to_string(),
            port,
            jetstream_config,
            operator_path: operator_path.to_string(),
            logging,
            leaf_nodes_remote,
            resolver_path: resolver_path.to_string(),
            server_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Generate the config file and run the server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = "./leaf_server.conf";
        let mut config_file = File::create(config_path)?;

        // Generate logging options
        let logging_config = format!(
            "debug: {}\ntrace: {}\nlogtime: {}\n",
            self.logging.debug, self.logging.trace, self.logging.longtime
        );

        // Generate the "leafnodes" block
        let leafnodes_config = self
            .leaf_nodes_remote
            .iter()
            .map(|leaf| {
                format!(
                    r#"
    {{
        url: "{}",
        credentials: "{}",
        account: "{}"
    }}
                "#,
                    leaf.url, leaf.credentials_path, leaf.account_key
                )
            })
            .collect::<Vec<String>>()
            .join(",\n");

        // Write the full config file
        write!(
            config_file,
            r#"
server_name: {}
listen: "{}:{}"

operator: "{}"
system_account: SYS

jetstream: {{
    domain: "leaf",
    store_dir: "{}",
    max_mem: {},
    max_file: {}
}}

leafnodes {{
    remotes = [
        {}
    ]
}}

include {}

{}
"#,
            self.name,
            self.host,
            self.port,
            self.operator_path,
            self.jetstream_config.store_dir,
            self.jetstream_config.max_memory_store,
            self.jetstream_config.max_file_store,
            leafnodes_config,
            self.resolver_path,
            logging_config
        )?;

        // Run the server with the generated config
        let child = Command::new("nats-server")
            .arg("-c")
            .arg(config_path)
            .spawn()
            .expect("Failed to start NATS server");

        println!("NATS Leaf Server is running at {}:{}", self.host, self.port);
        println!("NATS Leaf Server is running at {}:{}", self.host, self.port);

        // Store the process handle in the `server_handle`
        let mut handle = self.server_handle.lock().await;
        *handle = Some(child);

        Ok(())
    }

    /// Gracefully shut down the server
    pub async fn close(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut handle = self.server_handle.lock().await;

        if let Some(child) = handle.as_mut() {
            // Wait for the server process to finish
            let status = child.wait()?;
            println!("NATS server exited with status: {:?}", status);
        } else {
            println!("No running server to shut down.");
        }

        // Clear the server handle
        *handle = None;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_nats::ConnectOptions;
    use dotenv::dotenv;
    use futures::StreamExt;
    use tokio::time::{sleep, Duration};
    #[tokio::test]
    async fn test_leaf_server_run() {
        dotenv().ok();
        env_logger::init();

        let nsc_path =
            std::env::var("NSC_PATH").unwrap_or_else(|_| ".local/share/nats/nsc".to_string());

        let jetstream_config = JetStreamConfig {
            store_dir: "./tmp/leaf_store".to_string(),
            max_memory_store: 1024 * 1024 * 1024, // 1 GB
            max_file_store: 1024 * 1024 * 1024,   // 1 GB
        };

        let logging_options = LoggingOptions {
            debug: true, // NB: This logging is a blocking action, only run in non-prod
            trace: true, // NB: This logging is a blocking action, only run in non-prod
            longtime: false,
        };

        let leaf_remote_conn_url = "nats://127.0.0.1:7422";
        let test_sys_account_key = "ABTWA2WTWGMVT4KMGXBIBXNSKZ3N5TQEMJ3A2JDOFB7FANRPKZ2UUMNV";

        let leaf_nodes_remote = vec![LeafNodeRemote {
            url: leaf_remote_conn_url.to_string(),
            credentials_path: format!("{}/keys/creds/HOLO/SYS/sys.creds", nsc_path),
            account_key: test_sys_account_key.to_string(),
        }];

        // Create a new Leaf Server instance
        let leaf_server = LeafServer::new(
            "test_leaf_server",
            "127.0.0.1",
            4111,
            jetstream_config,
            &format!("{}/keys/stores/HOLO/HOLO.jwt", nsc_path),
            logging_options,
            leaf_nodes_remote,
            "./resolver/resolver.conf",
        );

        let leaf_server_clone = leaf_server.clone();
        // Start the Leaf Server in a separate thread
        let server_task = tokio::spawn(async move {
            leaf_server_clone
                .run()
                .await
                .expect("Failed to run Leaf Server");
        });

        // Wait for server to be ready
        sleep(Duration::from_secs(1)).await;

        log::info!("Running client connection test");

        // Connect a NATS client to the leaf server
        let leaf_client_conn_url = "127.0.0.1:4111";

        // Connect to the leaf server
        let conn_result = ConnectOptions::new()
            .name("test_leaf_client")
            .connect(leaf_client_conn_url)
            .await;

        assert!(conn_result.is_ok());
        let client = conn_result.expect("Failed to connect to NATS Leaf server");

        // Verify the connection is active
        assert_eq!(
            client.connection_state(),
            async_nats::connection::State::Connected
        );

        log::info!("Client successfully connected to the Leaf Server.");

        // Test JetStream capabilities
        let test_stream_name = "test_stream";
        let test_stream_subject = "test.subject";
        let js_context = async_nats::jetstream::new(client.clone());
        let stream = js_context
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: test_stream_name.to_string(),
                subjects: vec![test_stream_subject.to_string()],
                ..Default::default()
            })
            .await
            .expect("Failed to create stream");

        let stream_info = stream.get_info().await.expect("Failed to get stream info");
        assert_eq!(stream_info.config.name, "test_stream");
        assert!(stream_info
            .config
            .subjects
            .contains(&test_stream_subject.to_string()));

        let test_msg = "Hello, Leaf!";
        js_context
            .publish(test_stream_subject, test_msg.into())
            .await
            .expect("Failed to publish jetstream message.");

        // Subscribe to the stream
        let test_stream_consumer_name = "test_stream_consumer".to_string();
        let consumer = stream
            .get_or_create_consumer(
                &test_stream_consumer_name.to_string(),
                async_nats::jetstream::consumer::pull::Config {
                    durable_name: Some(test_stream_consumer_name),
                    ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                    ..Default::default()
                },
            )
            .await
            .expect("Failed to add consumer");

        let mut messages = consumer
            .messages()
            .await
            .expect("Failed to fetch messages")
            .take(1);

        let msg_option_result = messages.next().await;
        assert!(msg_option_result.is_some());

        let msg_result = msg_option_result.unwrap();
        assert!(msg_result.is_ok());

        let msg = msg_result.expect("No message received");
        assert_eq!(msg.payload, test_msg);

        // Shut down the client
        client
            .drain()
            .await
            .expect("Failed to drain and flush client");

        // Allow some time for cleanup
        sleep(Duration::from_secs(3)).await;

        log::info!("Client has shut down successfully");

        // Shut down the server
        if let Err(err) = leaf_server.close().await {
            log::error!("Failed to shut down Leaf Server.  Err:{:#?}", err);

            // Force the port to close
            Command::new("kill")
                .arg("-9")
                .arg("`lsof -t -i:4111`")
                .spawn()
                .expect("Failed to kill active Leaf Server port");
        }

        log::info!("Leaf Server has shut down successfully");

        // Await server task termination
        let _ = server_task.await;

        // Clean up temporary files
        std::fs::remove_file("./leaf_server.conf").expect("Failed to delete config file");
    }
}
