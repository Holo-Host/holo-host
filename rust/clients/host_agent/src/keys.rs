use anyhow::{anyhow, Context, Result};
use nkeys::KeyPair;
use data_encoding::BASE64URL_NOPAD;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use util_libs::nats_js_client;

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys")
            .field("host_keypair", &"[redacted]")
            .field("host_pubkey", &self.host_pubkey)
            .field("maybe_host_creds_path", &self.maybe_host_creds_path.is_some())
            .field("local_sys_keypair", if &self.local_sys_keypair.is_some(){&"[redacted]"} else { &false })
            .field("local_sys_pubkey", &self.local_sys_pubkey)
            .field("maybe_sys_creds_path", &self.maybe_sys_creds_path.is_some())
            .finish()
    }
}

pub struct Keys {
    host_keypair: KeyPair,
    pub host_pubkey: String,
    maybe_host_creds_path: Option<PathBuf>,
    local_sys_keypair: Option<KeyPair>,
    pub local_sys_pubkey: Option<String>,
    maybe_sys_creds_path: Option<PathBuf>,
}

impl Keys {
    pub fn new() -> Result<Self> {
        // let host_key_path = format!("{}/user_host_{}.nk", &get_nats_creds_by_nsc("HOLO", "HPOS", "host"), host_pubkey);
        let host_key_path = std::env::var("HOST_KEY_PATH").context("Cannot read HOST_KEY_PATH from env var")?;
        let host_kp = KeyPair::new_user();
        write_to_file(nats_js_client::get_file_path_buf(&host_key_path), host_kp.clone());
        let host_pk = host_kp.public_key();
        
        // let sys_key_path = format!("{}/user_sys_host_{}.nk", &get_nats_creds_by_nsc("HOLO", "HPOS", "host"), host_pubkey);
        let sys_key_path = std::env::var("SYS_KEY_PATH").context("Cannot read SYS_KEY_PATH from env var")?;
        let local_sys_kp = KeyPair::new_user();
        write_to_file(nats_js_client::get_file_path_buf(&sys_key_path), local_sys_kp.clone());
        let local_sys_pk = local_sys_kp.public_key();

        Ok(Self {
            host_keypair: host_kp,
            host_pubkey: host_pk,
            maybe_host_creds_path: None, 
            local_sys_keypair: Some(local_sys_kp),
            local_sys_pubkey: Some(local_sys_pk),
            maybe_sys_creds_path: None,
        })
    }

    pub fn try_from_storage(maybe_host_creds_path: &Option<PathBuf>, maybe_sys_creds_path: &Option<PathBuf>) -> Result<Option<Self>> {
        let host_key_path = std::env::var("HOST_KEY_PATH").context("Cannot read HOST_KEY_PATH from env var")?;
        let host_keypair = try_get_from_file(nats_js_client::get_file_path_buf(&host_key_path.clone()))?.ok_or_else(|| anyhow!("Host keypair not found at path {:?}", host_key_path))?;
        let host_pk = host_keypair.public_key();
        let sys_key_path = std::env::var("SYS_KEY_PATH").context("Cannot read SYS_KEY_PATH from env var")?;
        let host_creds_path = maybe_host_creds_path.to_owned().unwrap_or_else(|| nats_js_client::get_file_path_buf(
            &nats_js_client::get_nats_creds_by_nsc("HOLO", "HPOS", "host")
        ));
        let sys_creds_path = maybe_sys_creds_path.to_owned().unwrap_or_else(|| nats_js_client::get_file_path_buf(
            &nats_js_client::get_nats_creds_by_nsc("HOLO", "HPOS", "sys")
        ));
        let keys = match try_get_from_file(nats_js_client::get_file_path_buf(&sys_key_path))? {
            Some(kp) => {
                let local_sys_pk = kp.public_key();
                Self {
                    host_keypair,
                    host_pubkey:host_pk,
                    maybe_host_creds_path: None,
                    local_sys_keypair: Some(kp),
                    local_sys_pubkey: Some(local_sys_pk),
                    maybe_sys_creds_path: None
                }
            },
            None => {
                Self {
                    host_keypair,
                    host_pubkey: host_pk,
                    maybe_host_creds_path: None,
                    local_sys_keypair: None,
                    local_sys_pubkey: None,
                    maybe_sys_creds_path: None
                }
            }
        };
        
        return Ok(Some(keys.add_creds_paths(host_creds_path, sys_creds_path)?));
    }

    pub fn add_creds_paths(self, host_creds_file_path: PathBuf, sys_creds_file_path: PathBuf) -> Result<Self> {
        match host_creds_file_path.try_exists() {
            Ok(is_ok) => {
                if !is_ok {
                    return Err(anyhow!("Failed to locate host creds path. Found broken sym link. Path={:?}", host_creds_file_path));
                }
                match sys_creds_file_path.try_exists() {
                    Ok(is_ok) => {
                        if !is_ok {
                            return Err(anyhow!("Failed to locate sys creds path. Found broken sym link. Path={:?}", sys_creds_file_path));
                        }

                        Ok(Self {
                            maybe_host_creds_path: Some(host_creds_file_path),
                            maybe_sys_creds_path: Some(sys_creds_file_path),
                            ..self    
                        })
                    },
                    Err(e) => Err(anyhow!("Failed to locate sys creds path. Path={:?} Err={}", sys_creds_file_path, e))
                }
            },
            Err(e) => Err(anyhow!("Failed to locate host creds path. Path={:?} Err={}", host_creds_file_path, e))
        }
    }

    pub fn add_local_sys(self, sys_key_path: Option<PathBuf>) -> Result<Self> {
        let sys_key_path = sys_key_path.unwrap_or_else(|| nats_js_client::get_file_path_buf(
            &nats_js_client::get_nats_creds_by_nsc("HOLO", "HPOS", "sys")
        ));
        
        let local_sys_kp = try_get_from_file(sys_key_path.clone())?.unwrap_or_else(|| {
            let kp = KeyPair::new_user();
            write_to_file(sys_key_path, kp);
            KeyPair::new_user()
        });
        let local_sys_pk = local_sys_kp.public_key();

        Ok(Self {
            local_sys_keypair: Some(local_sys_kp),
            local_sys_pubkey: Some(local_sys_pk),
            ..self    
        })
    }

    pub fn get_host_creds_path(&self) -> Option<PathBuf> {
        self.maybe_host_creds_path.clone()
    }

    pub fn get_sys_creds_path(&self) -> Option<PathBuf> {
        self.maybe_sys_creds_path.clone()
    }

    pub fn host_sign(&self, payload: &[u8]) -> Result<String> {
        let signature = self
            .host_keypair
            .sign(payload)?;

        Ok(BASE64URL_NOPAD.encode(&signature))
    }
}

fn write_to_file(key_file_path: PathBuf, keypair: KeyPair) -> Result<()> {
    let seed = keypair.seed()?;
    let mut file = File::create(&key_file_path)?;
    file.write_all(seed.as_bytes())?;
    Ok(())
}

fn try_get_from_file(key_file_path: PathBuf) -> Result<Option<KeyPair>> {
    match key_file_path.try_exists() {
        Ok(link_is_ok) => {
            if !link_is_ok {
                return Err(anyhow!("Failed to read path {:?}. Found broken sym link.", key_file_path));
            }

            let mut key_file_content =
                File::open(&key_file_path).context(format!("Failed to open config file {:#?}", key_file_path))?;
        
            let mut kps = String::new();
            key_file_content.read_to_string(&mut kps)?;
            let kp = KeyPair::from_seed(&kps.trim())?;

            Ok(Some(kp))
        }
        Err(_) => {
            log::debug!("No user file found at {:?}.", key_file_path);
            Ok(None)
        }
    }
}
