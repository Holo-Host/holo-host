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
            .context("Failed to start NATS leaf server")?;

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
