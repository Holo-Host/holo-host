//! Ham (Holochain App Manager) provides utilities for managing Holochain applications.
//!
//! For an example look at the `main.rs` file.
//! ```

use anyhow::{Context, Result};
use derive_builder::Builder;
use holochain_client::{
    AdminWebsocket, AgentPubKey, AppWebsocket, AuthorizeSigningCredentialsPayload, CellInfo,
    ClientAgentSigner, ExternIO, GrantedFunctions, InstalledAppId,
};
use holochain_conductor_api::{AppInfo, AppInterfaceInfo};
use holochain_types::{
    app::{AppBundleSource, InstallAppPayload, RoleSettings},
    dna::{hash_type::Agent, HoloHash},
    prelude::{NetworkSeed, RoleName},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::Ipv4Addr, path::Path};
use url::Url;

pub mod exports {
    pub use holochain_client;
    pub use holochain_conductor_api;
    pub use holochain_types;
}

pub type ZomeName = String;
pub type ZomeCallFnName = String;
pub type MaybeZomeCallPayload = Option<String>;
pub type ZomeCalls = Vec<(ZomeName, (ZomeCallFnName, MaybeZomeCallPayload))>;

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
    pub async fn connect(addr: Ipv4Addr, admin_port: u16) -> Result<Self> {
        let admin = holochain_client::AdminWebsocket::connect((addr, admin_port))
            .await
            .context("Failed to connect to Holochain Admin interface")?;

        Ok(Self { admin_ws: admin })
    }

    /// Download a .happ file from a URL to a temporary location
    pub async fn download_happ_bytes(url: &Url) -> Result<bytes::Bytes> {
        log::debug!("Downloading happ from {url}"); // Add debug logging

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

        log::debug!("Downloaded {} bytes", bytes.len()); // Add debug logging

        Ok(bytes)
    }

    /// Install a .happ file from either a local path or URL with optional configuration
    pub async fn install_and_enable_happ(
        &mut self,
        happ_bytes: &[u8],
        maybe_network_seed: Option<NetworkSeed>,
        maybe_role_settings: Option<HashMap<RoleName, RoleSettings>>,
        maybe_installed_app_id: Option<InstalledAppId>,
    ) -> Result<(AppInfo, HoloHash<holochain_types::dna::hash_type::Agent>)> {
        // Generate a new agent key
        let agent_key = self.admin_ws.generate_agent_pub_key().await?;

        // Prepare installation payload
        let payload = {
            let source = AppBundleSource::Bytes(happ_bytes.to_vec());
            InstallAppPayload {
                agent_key: Some(agent_key.clone()),
                installed_app_id: maybe_installed_app_id,
                source,
                network_seed: maybe_network_seed,
                roles_settings: maybe_role_settings,
                ignore_genesis_failure: false,
                allow_throwaway_random_agent_key: false,
            }
        };

        // Install and enable the app
        let app_info = self
            .admin_ws
            .install_app(payload)
            .await
            .context("Failed to install app")?;
        self.admin_ws
            .enable_app(app_info.installed_app_id.clone())
            .await
            .context("Failed to enable app")?;

        Ok((app_info, agent_key))
    }

    /// Looks for an installed app id and returns its AppInfo upon finding it.
    pub async fn find_installed_app(
        &self,
        installed_app_id: &InstalledAppId,
    ) -> anyhow::Result<Option<(AppInfo, AgentPubKey, Vec<AppInterfaceInfo>)>> {
        let maybe_app_info = self.admin_ws.list_apps(None).await.map(|apps| {
            apps.into_iter().find_map(|app_info| {
                if &app_info.installed_app_id == installed_app_id {
                    let agent_pubkey = app_info.agent_pub_key.clone();
                    Some((app_info, agent_pubkey))
                } else {
                    None
                }
            })
        })?;

        let app_interfaces = self
            .admin_ws
            .list_app_interfaces()
            .await?
            .into_iter()
            .filter(|app_interface| {
                app_interface.installed_app_id.as_ref() == Some(installed_app_id)
            })
            .collect::<Vec<_>>();

        Ok(maybe_app_info.map(|(app_info, pubkey)| (app_info, pubkey, app_interfaces)))
    }

    pub async fn call_zomes(
        &self,
        ham_state: HamState,
        zome_calls: ZomeCalls,
    ) -> anyhow::Result<HashMap<String, ExternIO>> {
        let token_issued = self
            .admin_ws
            .issue_app_auth_token(ham_state.app_info.installed_app_id.clone().into())
            .await
            .context("issuing token")?;

        // prepare the signer, which will receive the credentials for each cell in the subsequent loop
        let signer = ClientAgentSigner::default();

        let addr = Ipv4Addr::LOCALHOST;
        let port = ham_state.app_ws_port;
        let app_ws = AppWebsocket::connect((addr, port), token_issued.token, signer.clone().into())
            .await
            .context("connecting to app websocket at {addr}:{port}")?;

        let mut all_data = HashMap::<String, ExternIO>::new();

        for (cell_name, cell_infos) in ham_state.app_info.cell_info {
            // for each cell call the init zome function
            for cell_info in cell_infos {
                println!("cell_info: {:#?}", &cell_info);

                let cell_id = match &cell_info {
                    CellInfo::Provisioned(c) => c.cell_id.clone(),
                    CellInfo::Cloned(c) => c.cell_id.clone(),
                    other => anyhow::bail!("Invalid cell type: {other:?}"),
                };

                let credentials = self
                    .admin_ws
                    // this writes a capgrant onto the source-chain to grant zomecall access to the `AgentPubKey` specified in the cell
                    .authorize_signing_credentials(AuthorizeSigningCredentialsPayload {
                        cell_id: cell_id.clone(),
                        functions: Some(GrantedFunctions::All),
                    })
                    .await
                    .context(format!(
                        "authorizing signing credentials for cell_id {}",
                        &cell_id,
                    ))?;
                signer.add_credentials(cell_id.clone(), credentials);

                'given_zome_calls: for (zome_name, (zome_fn_name, maybe_zome_fn_payload)) in
                    zome_calls.iter()
                {
                    print!(
                            "[{cell_name}/{zome_name}] processing {zome_fn_name} @ {zome_name} with payload {maybe_zome_fn_payload:?}.. ",
                        );

                    let cell_name = match &cell_info {
                        CellInfo::Provisioned(provisioned_cell) => provisioned_cell.name.clone(),
                        CellInfo::Cloned(cloned_cell) => cloned_cell.name.clone(),
                        CellInfo::Stem(stem_cell) => stem_cell.clone().name.unwrap_or_default(),
                    };
                    if !(zome_name.starts_with(&cell_name) || cell_name.starts_with(zome_name)) {
                        println!(
                            "skipping cell with name {cell_name} for call to zome {zome_name}"
                        );
                        continue 'given_zome_calls;
                    }

                    let payload = if let Some(payload) = maybe_zome_fn_payload.clone() {
                        ExternIO::encode(payload)
                    } else {
                        ExternIO::encode(())
                    }
                    .context("encoding payload")?;

                    match app_ws
                        .call_zome(
                            cell_id.clone().into(),
                            zome_name.clone().into(),
                            zome_fn_name.clone().into(),
                            payload,
                        )
                        .await
                        // .map(|io| -> Result<Vec<String>, _> { io.decode() })
                    {
                        Ok(data) => {
                                let key = format!("{cell_name}_{zome_name}_{zome_fn_name}");

                                all_data.insert(key, data);
                        }
                        Err(e) => log::error!("error: {e}"),
                    };
                }
            }
        }

        Ok(all_data)
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
