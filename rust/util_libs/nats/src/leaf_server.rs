/* --------
    This file contains the configuration required to set up a NATS Leaf Server with the "Operator JWT" auth approach.
    NB: This setup expects the `nats-server` binary to be locally installed and accessible.
-------- */
use anyhow::Context;
use async_nats::ServerAddr;
use serde::Serialize;
use serde_with::skip_serializing_none;
use std::fmt::Debug;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::str::FromStr;
// Child, Command,
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use url::Host;

// pub const LEAF_SERVER_CONFIG_PATH: &str = "test_leaf_server.conf";
pub const LEAF_SERVER_DEFAULT_LISTEN_PORT: u16 = 4222;

#[derive(Serialize, Debug, Clone)]
pub struct JetStreamConfig {
    pub store_dir: PathBuf,
    pub max_memory_store: u64,
    pub max_file_store: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LoggingOptions {
    pub debug: bool,
    pub trace: bool,
    pub logtime: bool,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize)]
pub struct LeafNodeRemote {
    // TODO: Leafnode URL (URL protocol should be nats-leaf).
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
    nats_config: NatsConfig,
    pub config_path: String,
    server_handle: Arc<Mutex<Option<Child>>>,
    server_command: Arc<Mutex<Option<tokio::process::Command>>>,
}

#[derive(Clone, Serialize, Debug)]
struct NatsConfig {
    // needs to be unique
    // [1465412] [ERR] 65.108.153.204:443 - lid_ws:5 - Leafnode Error 'Duplicate Remote LeafNode Connection'
    server_name: Option<String>,
    #[serde(serialize_with = "serialize_host")]
    host: Host<String>,
    port: u16,
    max_payload: Option<u64>,
    jetstream: JetStreamConfig,
    leafnodes: LeafNodes,
    #[serde(flatten)]
    logging: LoggingOptions,
    // debug: bool,
    // trace: bool,
    // logtime: bool,
}

fn serialize_host<S>(t: &Host<String>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let serialized = t.to_string().serialize(s)?;

    Ok(serialized)
}

#[derive(Debug, Clone, Serialize)]
struct LeafNodes {
    remotes: Vec<LeafNodeRemote>,
}

impl LeafServer {
    // Instantiate a new leaf server
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        server_name: Option<&str>,
        new_config_path: &str,
        host: Host<String>,
        port: u16,
        max_payload: Option<u64>,
        jetstream_config: JetStreamConfig,
        logging: LoggingOptions,
        leaf_node_remotes: Vec<LeafNodeRemote>,
    ) -> Self {
        Self {
            nats_config: NatsConfig {
                server_name: server_name.map(ToString::to_string),
                host,
                port,
                max_payload,
                jetstream: jetstream_config,
                logging,
                leafnodes: LeafNodes {
                    remotes: leaf_node_remotes,
                },
            },
            config_path: new_config_path.to_string(),
            server_handle: Default::default(),
            server_command: Default::default(),
        }
    }

    /// Generate the config file and run the server
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_file = File::create(&self.config_path)?;

        let config = self.nats_config.clone();
        let config_str = serde_json::to_string_pretty(&config)?;

        log::trace!("NATS leaf config:\n{config_str}");

        config_file
            .write_all(config_str.as_bytes())
            .context("writing config to config at {config_path}")?;

        {
            // Run the server with the generated config
            let mut server_command = Command::new("nats-server");
            server_command
                .arg("-c")
                .arg(&self.config_path)
                // TODO: make this configurable and give options to log it to a seperate log file
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            // Store the process command to protect it from being dropped
            *self.server_command.lock().await = Some(server_command);

            let child = self
                .server_command
                .lock()
                .await
                .as_mut()
                .map(|server_command| {
                    server_command
                        .kill_on_drop(true)
                        .spawn()
                        .context("Failed to start NATS server")
                })
                .transpose()?
                .ok_or_else(|| anyhow::anyhow!("server_command must be there at this point"))?;

            // Store the process handle in the `server_handle`
            *self.server_handle.lock().await = Some(child);
        }

        // TODO: wait for a readiness indicator

        log::info!(
            "NATS Leaf Server is running at {}",
            self.server_addr()?.into_inner()
        );

        Ok(())
    }

    /// Gracefully shut down the server
    pub async fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut handle = self.server_handle.lock().await.take();

            if let Some(child) = handle.as_mut() {
                // Wait for the server process to finish
                let _ = child
                    .kill()
                    .await
                    .map(|e| log::warn!("error terminating the server: {e:?}"));
                let status = child.wait().await?;
                log::info!("NATS server exited with status: {:?}", status);
            } else {
                log::info!("No running server to shut down.");
            }
        }

        // Clear the server command and handle
        self.server_handle = Default::default();
        self.server_command = Default::default();

        Ok(())
    }

    pub fn server_addr(&self) -> anyhow::Result<ServerAddr> {
        Ok(ServerAddr::from_str(&format!(
            "{}:{}",
            self.nats_config.host, self.nats_config.port
        ))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaf_server_with_domain() {
        let config = JetStreamConfig {
            store_dir: PathBuf::from("/tmp/test"),
            max_memory_store: 1024 * 1024 * 1024,
            max_file_store: 1024 * 1024 * 1024,
            domain: Some("holo".to_string()),
        };

        let config_json = serde_json::to_string_pretty(&config).unwrap();
        assert!(config_json.contains("\"domain\": \"holo\""));
        println!("Generated config: {}", config_json);
    }

    #[test]
    fn test_leaf_server_without_domain() {
        let config = JetStreamConfig {
            store_dir: PathBuf::from("/tmp/test"),
            max_memory_store: 1024 * 1024 * 1024,
            max_file_store: 1024 * 1024 * 1024,
            domain: None,
        };

        let config_json = serde_json::to_string_pretty(&config).unwrap();
        assert!(!config_json.contains("\"domain\""));
        println!("Generated config: {}", config_json);
    }
}
