use ed25519_dalek::SigningKey;
use hpos_config_core::public_key;
use hpos_config_core::Config;
use hpos_config_seed_bundle_explorer::unlock;
use std::env;
use std::fs::File;
use std::path::Path;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

// Configuration for the Hoster - contains identity information
pub struct HosterConfig {
    // Email address of the hoster/admin
    pub email: String,
    // Ed25519 signing key pair
    #[allow(dead_code)]
    keypair: SigningKey,
    // Holochain-encoded agent pubkey
    pub hc_pubkey: String,
    // Base36-encoded unique identifier for the HoloPort
    #[allow(dead_code)]
    pub holoport_id: String,
}

impl HosterConfig {
    /*
    Creates a new HosterConfig by loading configuration from env vars
    # Errors
        Returns an error if:
        - Required env vars are not set
        - Config files cannot be read
        - Password cannot be used to unlock the device bundle
        - Config format is unsupported
    */
    pub async fn new() -> HostAgentResult<Self> {
        let (keypair, email) = try_from_config().await?;
        let verifying_key = keypair.verifying_key();
        let hc_pubkey = public_key::to_holochain_encoded_agent_key(&verifying_key);
        let holoport_id = public_key::to_base36_id(&verifying_key);

        Ok(Self {
            email,
            keypair,
            hc_pubkey,
            holoport_id,
        })
    }
}

// Attempts to load configuration from the files specified by env vars
async fn try_from_config() -> HostAgentResult<(SigningKey, String)> {
    // Load and validate configuration file path
    let config_path = env::var("HPOS_CONFIG_PATH")?;

    // Validate config file exists and is readable
    if !Path::new(&config_path).exists() {
        return Err(HostAgentError::service_failed(
            "configuration file access",
            &format!("Configuration file does not exist: {}", config_path),
        ));
    }

    let config_file = File::open(&config_path)?;

    // Load and validate password file path
    let password_file_path = env::var("DEVICE_SEED_DEFAULT_PASSWORD_FILE")?;

    // Validate password file exists
    if !Path::new(&password_file_path).exists() {
        return Err(HostAgentError::service_failed(
            "password file access",
            &format!("Password file does not exist: {}", password_file_path),
        ));
    }

    // Read password with basic validation
    let password = std::fs::read_to_string(&password_file_path)?;

    if password.trim().is_empty() {
        return Err(HostAgentError::service_failed(
            "password validation",
            &format!("Password file is empty: {}", password_file_path),
        ));
    }

    let config: Config = serde_json::from_reader(config_file)?;

    match config {
        Config::V2 {
            device_bundle,
            settings,
            ..
        } => {
            // Unlock device bundle with password
            let signing_key = unlock(&device_bundle, Some(password.trim().to_string()))
                .await
                .map_err(|e| {
                    HostAgentError::service_failed(
                        "device bundle unlock",
                        &format!(
                            "Failed to unlock device bundle from configuration file '{}': {}",
                            config_path, e
                        ),
                    )
                })?;

            Ok((signing_key, settings.admin.email))
        }
        Config::V1 { .. } => Err(HostAgentError::service_failed(
            "configuration version",
            &format!(
                "Unsupported configuration version: V1. Please upgrade to V2 format. File: {}",
                config_path
            ),
        )),
        _ => Err(HostAgentError::service_failed(
            "configuration version",
            &format!(
                "Unsupported or unknown configuration version in file: {}",
                config_path
            ),
        )),
    }
}
