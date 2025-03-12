//! Ham (Holochain App Manager) provides utilities for managing Holochain applications.
//!
//! For an example look at the `main.rs` file.
//! ```

use anyhow::{Context, Result};
use derive_builder::Builder;
use holochain_client::AdminWebsocket;
use holochain_conductor_api::AppInfo;
use holochain_types::{
    app::{AppBundleSource, InstallAppPayload},
    dna::{hash_type::Agent, HoloHash},
    prelude::NetworkSeed,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use url::Url;

/// Manages Holochain application installation and lifecycle
pub struct Ham {
    pub admin_ws: AdminWebsocket,
}

#[derive(Debug, Serialize, Deserialize, Builder)]
// because not all field types impl Clone
#[builder(pattern = "owned")]
pub struct HamState {
    pub agent_key: HoloHash<Agent>,
    pub app_ws_port: u16,
    // pub app_authentication_token_payload: AppAuthenticationToken,
    pub app_info: AppInfo,
}

impl HamState {
    pub fn persist(&self, path: &Path) -> anyhow::Result<()> {
        let file = std::fs::File::create(path).context(format!("opening {path:?}"))?;

        serde_json::to_writer_pretty(&file, self)
            .context(format!("serializing HamSate from {path:?}"))?;

        Ok(())
    }

    pub fn from_state_file(path: &Path) -> anyhow::Result<Option<Self>> {
        if !std::fs::exists(path).context(format!("checking for existence of {path:?}"))? {
            return Ok(None);
        }

        let file = std::fs::File::open(path).context(format!("opening {path:?}"))?;

        let new_self = serde_json::from_reader(&file)
            .context(format!("deserializing HamSate from {path:?}"))?;

        Ok(Some(new_self))
    }
}

impl Ham {
    /// Connect to a running Holochain conductor's admin interface
    pub async fn connect(admin_port: u16) -> Result<Self> {
        use std::net::Ipv4Addr;
        let admin = holochain_client::AdminWebsocket::connect((Ipv4Addr::LOCALHOST, admin_port))
            .await
            .context("Failed to connect to admin interface")?;

        Ok(Self { admin_ws: admin })
    }

    /// Download a .happ file from a URL to a temporary location
    pub async fn download_happ(url: &Url) -> Result<PathBuf> {
        // Create a temporary directory that won't be deleted when the TempDir is dropped
        let temp_dir = tempfile::Builder::new()
            .prefix("ham-download-")
            .tempdir()
            .context("Failed to create temporary directory")?;

        // Keep the TempDir alive by converting it to a PathBuf
        let temp_path = temp_dir.into_path();

        let file_name = url
            .path_segments()
            .and_then(|segments| segments.last())
            .unwrap_or("downloaded.happ");

        let file_path = temp_path.join(file_name);

        println!("Downloading happ to: {:?}", file_path); // Add debug logging

        let response = reqwest::get(url.as_str())
            .await
            .context("Failed to download happ file")?;

        // Check if the download was successful
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to download happ file: HTTP {}",
                response.status()
            ));
        }

        let bytes = response
            .bytes()
            .await
            .context("Failed to read response body")?;

        println!("Downloaded {} bytes", bytes.len()); // Add debug logging

        std::fs::write(&file_path, bytes).context("Failed to write happ file")?;

        // Verify the file was written
        if !file_path.exists() {
            return Err(anyhow::anyhow!("Failed to verify downloaded file exists"));
        }

        Ok(file_path)
    }

    pub async fn get_happ_bytes<P: AsRef<Path>>(happ_source: P) -> Result<Box<[u8]>> {
        let happ_path = if let Ok(url) = Url::parse(happ_source.as_ref().to_str().unwrap_or("")) {
            Self::download_happ(&url).await?
        } else {
            happ_source.as_ref().to_path_buf()
        };

        let bytes = std::fs::read(happ_path)?;

        Ok(bytes.into_boxed_slice())
    }

    /// Install a .happ file from either a local path or URL with optional configuration
    pub async fn install_and_enable_happ(
        &mut self,
        happ_bytes: &[u8],
        maybe_network_seed: Option<NetworkSeed>,
    ) -> Result<(AppInfo, HoloHash<holochain_types::dna::hash_type::Agent>)> {
        // Generate a new agent key
        let agent_key = self.admin_ws.generate_agent_pub_key().await?;

        // Prepare installation payload
        let payload = {
            let bundle = holochain_types::app::AppBundle::decode(happ_bytes)
                .context("decoding happ_bytes into an AppBundle".to_string())?;

            let source = AppBundleSource::Bundle(bundle);
            InstallAppPayload {
                agent_key: Some(agent_key.clone()),
                installed_app_id: None,
                source,
                network_seed: maybe_network_seed,
                roles_settings: None,
                ignore_genesis_failure: false,
                allow_throwaway_random_agent_key: false,
            }
        };

        // Install and enable the app
        let app_info = self
            .admin_ws
            .install_app(payload)
            .await
            .expect("Failed to install app");
        self.admin_ws
            .enable_app(app_info.installed_app_id.clone())
            .await
            .context("Failed to enable app")?;

        Ok((app_info, agent_key))
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use holochain_env_setup::environment::setup_environment;
//     use tempfile::tempdir;

//     const TEST_HAPP_URL: &str =
//         "https://github.com/holochain/ziptest/releases/download/ziptest-v0.1.0/ziptest.happ";

//     #[tokio::test]
//     async fn test_app_installation() -> Result<()> {
//         // Initialize logging for better debugging
//         tracing_subscriber::fmt::init();
//         // Create temporary directories for the test
//         let tmp_dir = tempdir()?.into_path();
//         let log_dir = tmp_dir.join("log");
//         std::fs::create_dir_all(&log_dir)?;
//         println!("Log directory created: {:?}", log_dir);
//         // Setup the Holochain environment (starts conductor & lair)
//         let _env = setup_environment(&tmp_dir, &log_dir, None, None).await?;
//         println!("Environment setup complete...");
//         // Wait a moment for the conductor to be ready
//         tokio::time::sleep(std::time::Duration::from_secs(1)).await;

//         println!("Connecting to admin interface...");
//         let mut manager = Ham::connect(4444).await?;
//         println!("Installing app from {}...", TEST_HAPP_URL);
//         let app_info = manager
//             .install_and_enable_with_default_agent(TEST_HAPP_URL, None)
//             .await?;
//         println!("App installed: {:?}", app_info);
//         assert!(!app_info.installed_app_id.is_empty());

//         Ok(())
//     }

//     #[tokio::test]
//     async fn test_happ_download() -> Result<()> {
//         let url = Url::parse(TEST_HAPP_URL).unwrap();
//         let downloaded_path = Ham::download_happ(&url).await?;

//         assert!(downloaded_path.exists(), "Downloaded file should exist");
//         assert!(
//             downloaded_path.metadata()?.len() > 0,
//             "Downloaded file should not be empty"
//         );

//         // Read a few bytes to verify it's a valid file
//         let content = std::fs::read(&downloaded_path)?;
//         println!("Downloaded file size: {} bytes", content.len());

//         Ok(())
//     }
// }
