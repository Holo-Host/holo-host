use anyhow::{anyhow, Context, Result};
use ed25519_dalek::*;
use hpos_config_core::public_key;
use hpos_config_core::Config;
use hpos_config_seed_bundle_explorer::unlock;
use std::env;
use std::fs::File;

pub struct HosterConfig {
    pub email: String,
    #[allow(dead_code)]
    keypair: SigningKey,
    pub hc_pubkey: String,
    #[allow(dead_code)]
    pub holoport_id: String,
}

impl HosterConfig {
    pub async fn new() -> Result<Self> {
        println!(">>> inside Hoster Config new fn..");

        let (keypair, email) = get_from_config().await?;
        println!(">>> inside Hoster Config new fn : keypair={:#?}", keypair);

        let hc_pubkey = public_key::to_holochain_encoded_agent_key(&keypair.verifying_key());
        println!(">>> inside Hoster Config new fn : hc_pubkey={}", hc_pubkey);

        let holoport_id = public_key::to_base36_id(&keypair.verifying_key());
        println!(">>> inside Hoster Config new fn : holoport_id={}", holoport_id);

        Ok(Self {
            email,
            keypair,
            hc_pubkey,
            holoport_id,
        })
    }
}

async fn get_from_config() -> Result<(SigningKey, String)> {
    println!("inside config_path...");

    let config_path =
        env::var("HPOS_CONFIG_PATH").context("Cannot read HPOS_CONFIG_PATH from env var")?;
    
    let password = env::var("DEVICE_SEED_DEFAULT_PASSWORD")
        .context("Cannot read bundle password from env var")?;

    let config_file =
        File::open(&config_path).context(format!("Failed to open config file {}", config_path))?;

    match serde_json::from_reader(config_file)? {
        Config::V2 {
            device_bundle,
            settings,
            ..
        } => {
            // take in password
            let signing_key = unlock(&device_bundle, Some(password))
                .await
                .context(format!(
                    "unable to unlock the device bundle from {}",
                    &config_path
                ))?;
                println!(">>> inside config-path new fn : signing_key={:#?}", signing_key);
            Ok((signing_key, settings.admin.email))
        }
        _ => Err(anyhow!("Unsupported version of hpos config")),
    }
}
