/* --------
    This file contains the configuration required to set up a NATS Leaf Server with the "Operator JWT" auth approach.
    NB: This setup expects the `nats-server` binary to be locally installed and accessible.
-------- */
use anyhow::Context;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

pub const LEAF_SERVER_CONFIG_PATH: &str = "test_leaf_server.conf";
pub const LEAF_SERVER_DEFAULT_LISTEN_PORT: u16 = 4111;

#[derive(Serialize, Debug, Clone)]
pub struct JetStreamConfig {
    pub store_dir: PathBuf,
    pub max_memory_store: u64,
    pub max_file_store: u64,
}

#[derive(Debug, Clone)]
pub struct LoggingOptions {
    pub debug: bool,
    pub trace: bool,
    pub longtime: bool,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
pub struct LeafNodeRemote {
    pub url: String,
    pub credentials: Option<PathBuf>,
    pub tls: LeafNodeRemoteTlsConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeafNodeRemoteTlsConfig {
    pub insecure: bool,
    pub handshake_first: bool,
}

impl Default for LeafNodeRemoteTlsConfig {
    fn default() -> Self {
        Self {
            insecure: false,
            handshake_first: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeafServer {
    // needs to be unique
    // [1465412] [ERR] 65.108.153.204:443 - lid_ws:5 - Leafnode Error 'Duplicate Remote LeafNode Connection'
    pub name: Option<String>,
    pub config_path: String,
    host: String,
    pub port: u16,
    jetstream_config: JetStreamConfig,
    pub logging: LoggingOptions,
    leaf_node_remotes: Vec<LeafNodeRemote>,
    server_handle: Arc<Mutex<Option<Child>>>,
}

// TODO: consider merging this with the `LeafServer` struct
#[derive(Serialize)]
struct NatsConfig {
    server_name: Option<String>,
    host: String,
    port: u16,
    jetstream: JetStreamConfig,
    leafnodes: LeafNodes,
    debug: bool,
    trace: bool,
    logtime: bool,
}

#[derive(Serialize)]
struct LeafNodes {
    remotes: Vec<LeafNodeRemote>,
}

impl LeafServer {
    // Instantiate a new leaf server
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        server_name: Option<&str>,
        new_config_path: &str,
        host: &str,
        port: u16,
        jetstream_config: JetStreamConfig,
        logging: LoggingOptions,
        leaf_node_remotes: Vec<LeafNodeRemote>,
    ) -> Self {
        Self {
            name: server_name.map(ToString::to_string),
            config_path: new_config_path.to_string(),
            host: host.to_string(),
            port,
            jetstream_config,
            logging,
            leaf_node_remotes,
            server_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Generate the config file and run the server
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_file = File::create(&self.config_path)?;

        let config = NatsConfig {
            server_name: self.name.clone(),
            host: self.host.clone(),
            port: self.port,
            jetstream: self.jetstream_config.clone(),
            leafnodes: LeafNodes {
                remotes: self.leaf_node_remotes.clone(),
            },

            debug: self.logging.debug,
            trace: self.logging.trace,
            logtime: self.logging.longtime,
        };

        let config_str = serde_json::to_string_pretty(&config)?;

        log::trace!("NATS leaf config:\n{config_str}");

        config_file
            .write_all(config_str.as_bytes())
            .context("writing config to config at {config_path}")?;

        // Run the server with the generated config
        let child = Command::new("nats-server")
            .arg("-c")
            .arg(&self.config_path)
            // TODO: make this configurable and give options to log it to a seperate log file
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to start NATS server")?;

        // TODO: wait for a readiness indicator
        std::thread::sleep(std::time::Duration::from_millis(100));

        log::info!("NATS Leaf Server is running at {}:{}", self.host, self.port);

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
            log::info!("NATS server exited with status: {:?}", status);
        } else {
            log::info!("No running server to shut down.");
        }

        // Clear the server handle
        *handle = None;

        Ok(())
    }
}

#[cfg(feature = "tests_integration_nats")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::nats_types;
    use async_nats::ConnectOptions;
    use dotenv::dotenv;
    use futures::StreamExt;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use tokio::time::{sleep, Duration};

    const TMP_JS_DIR: &str = "./tmp";
    const TEST_AUTH_DIR: &str = "./tmp/test-auth";
    const OPERATOR_NAME: &str = "test-operator";
    const USER_ACCOUNT_NAME: &str = "host-account";
    const USER_NAME: &str = "host-user";
    const NEW_LEAF_CONFIG_PATH: &str = "./test_configs/leaf_server.conf";
    // NB: if changed, the resolver file path must also be changed in the `hub-server.conf` iteself as well.
    const RESOLVER_FILE_PATH: &str = "./test_configs/resolver.conf";
    const HUB_SERVER_CONFIG_PATH: &str = "./test_configs/hub_server.conf";

    fn gen_test_agents(jwt_server_url: &str) {
        if Path::new(TEST_AUTH_DIR).exists() {
            fs::remove_dir_all(TEST_AUTH_DIR)
                .expect("Failed to delete already existing test auth dir");
        }
        fs::create_dir_all(TEST_AUTH_DIR).expect("Failed to create test auth dir");

        // Create operator and sys account/user
        Command::new("nsc")
            .args(["env", "-s", TEST_AUTH_DIR])
            .output()
            .expect("Failed to set env to the test auth dir");

        Command::new("nsc")
            .args(["add", "operator", "-n", OPERATOR_NAME, "--sys"])
            .output()
            .expect("Failed to add operator");

        Command::new("nsc")
            .args([
                "edit",
                "operator",
                "--account-jwt-server-url",
                &format!("nats://{}", jwt_server_url),
            ])
            .output()
            .expect("Failed to create edit operator");

        // Create host account (with js enabled)
        Command::new("nsc")
            .args(["add", "account", USER_ACCOUNT_NAME])
            .output()
            .expect("Failed to add acccount");

        Command::new("nsc")
            .args(["edit", "account", USER_ACCOUNT_NAME])
            .args([
                "--sk generate",
                "--js-streams -1",
                "--js-consumer -1",
                "--js-mem-storage 1G",
                "--js-disk-storage 512",
            ])
            .output()
            .expect("Failed to create edit account");

        // Create user for host account
        Command::new("nsc")
            .args(["add", "user", USER_NAME])
            .args(["--account", USER_ACCOUNT_NAME])
            .output()
            .expect("Failed to add user");

        // Fetch SYS account public key
        let sys_account_output = Command::new("nsc")
            .args(["describe", "account", "--json", "SYS"])
            .output()
            .expect("Failed to output sys account claim")
            .stdout;

        let sys_account_claim: nats_types::Claims = serde_json::from_slice(&sys_account_output)
            .expect("Failed to deserialize sys account info into account jwt");
        let sys_account_pubkey = sys_account_claim.sub;

        log::info!("SYS ACCOUNT PUBKEY : {:#?}", sys_account_pubkey);

        // Generate resolver file and create resolver file
        Command::new("nsc")
            .arg("generate")
            .arg("config")
            .arg("--nats-resolver")
            .arg("sys-account SYS")
            .arg("--force")
            .arg(format!("--config-file {}", RESOLVER_FILE_PATH))
            .output()
            .expect("Failed to create resolver config file");

        // Push auth updates to hub server
        Command::new("nsc")
            .arg("push -A")
            .output()
            .expect("Failed to create resolver config file");
    }

    #[tokio::test]
    async fn test_leaf_server_run() {
        dotenv().ok();
        env_logger::init();

        let leaf_server_remote_conn_url = "nats://127.0.0.1:7422";
        let leaf_client_conn_domain = "127.0.0.1";
        let leaf_client_conn_port = 4333;
        let leaf_client_conn_url = format!("{}:{}", leaf_client_conn_domain, leaf_client_conn_port);

        let nsc_path =
            std::env::var("NSC_PATH").unwrap_or_else(|_| ".local/share/nats/nsc".to_string());

        if Path::new(TMP_JS_DIR).exists() {
            fs::remove_dir_all(TMP_JS_DIR)
                .expect("Failed to delete already existing tmp jetstream store dir");
        }

        let jetstream_config = JetStreamConfig {
            store_dir: format!("{}/leaf_store", TMP_JS_DIR),
            max_memory_store: 1024 * 1024 * 1024, // 1 GB
            max_file_store: 1024 * 1024 * 1024,   // 1 GB
        };

        let logging_options = LoggingOptions {
            debug: true, // NB: This logging is a blocking action, only run in non-prod
            trace: true, // NB: This logging is a blocking action, only run in non-prod
            longtime: false,
        };

        gen_test_agents(&leaf_client_conn_url);

        let leaf_node_remotes = vec![LeafNodeRemote {
            // sys account user (automated)
            url: leaf_server_remote_conn_url.to_string(),
            credentials_path: Some(format!(
                "{}/keys/creds/{}/SYS/sys.creds",
                nsc_path, OPERATOR_NAME
            )),
        }];

        // Create a new Leaf Server instance
        let leaf_server = LeafServer::new(
            "test_leaf_server",
            NEW_LEAF_CONFIG_PATH,
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
        // Wait for Leaf Server to be ready
        sleep(Duration::from_secs(1)).await;

        log::info!("Spawning Hub server");
        let hub_server_handle: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        // Start the Hub Server in a separate thread
        tokio::spawn(async move {
            // Run the Hub Server with the (prexisting) config
            let child = Command::new("nats-server")
                .arg("-c")
                .arg(HUB_SERVER_CONFIG_PATH)
                .spawn()
                .expect("Failed to start Hub server");

            log::info!("Hub Server is running...");

            // Store the process handle in the `server_handle`
            let mut handle = hub_server_handle.lock().await;
            *handle = Some(child);
        });
        // Wait for Hub server to be ready
        sleep(Duration::from_secs(1)).await;

        log::info!("Running client connection test");
        // Connect to the leaf server
        let conn_result = ConnectOptions::new()
            .name("test_client")
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

        // Test js on Leaf Client:
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

        // Test client publishing to js stream
        let test_msg = "Hello, Leaf!";
        js_context
            .publish(test_stream_subject, test_msg.into())
            .await
            .expect("Failed to publish jetstream message.");

        // Force shut down the Hub Server (note: leaf server run on port LEAF_SERVER_DEFAULT_LISTEN_PORT)
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

        // Shut down the Leaf server
        if let Err(err) = leaf_server.close().await {
            log::error!("Failed to shut down Leaf Server.  Err:{:#?}", err);

            // Force the port to close
            // TODO(techdebt): use the command child handle to terminate the process.
            Command::new("kill")
                .arg("-9")
                .arg(format!("`lsof -t -i:{}`", leaf_client_conn_port))
                .spawn()
                .expect("Failed to spawn kill command")
                .wait()
                .expect("Failed to kill active Leaf Server port");
        }
        log::info!("Leaf Server has shut down successfully");

        // Force shut down the Hub Server (note: leaf server run on port LEAF_SERVER_DEFAULT_LISTEN_PORT)
        // TODO(techdebt): use the command child handle to terminate the process.
        Command::new("kill")
            .arg("-9")
            .arg(format!("`lsof -t -i:{LEAF_SERVER_DEFAULT_LISTEN_PORT}`"))
            .spawn()
            .expect("Error spawning kill command")
            .wait()
            .expect("Failed to kill active Leaf Server port");
        log::info!("Hub Server has shut down successfully");

        // Await server task termination
        let _ = leaf_server_task.await;

        // Clean up temporary dir & files
        std::fs::remove_dir_all(TEST_AUTH_DIR).expect("Failed to delete config file");
        std::fs::remove_dir_all(TMP_JS_DIR).expect("Failed to delete config file");
        std::fs::remove_file(NEW_LEAF_CONFIG_PATH).expect("Failed to delete config file");
        std::fs::remove_file(RESOLVER_FILE_PATH).expect("Failed to delete config file");
    }
}
