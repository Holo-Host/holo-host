use anyhow::{anyhow, Context, Result};
use data_encoding::BASE64URL_NOPAD;
use nkeys::KeyPair;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use util_libs::nats_js_client::{get_nats_creds_by_nsc, get_local_creds_path};

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let creds_type = match self.creds {
            AuthCredType::Guard(_) => "Guard",
            AuthCredType::Authenticated(_) => "Authenticated",
        };
        f.debug_struct("Keys")
            .field("host_keypair", &"[redacted]")
            .field("host_pubkey", &self.host_pubkey)
            .field(
                "local_sys_keypair",
                if self.local_sys_keypair.is_some() {
                    &"[redacted]"
                } else {
                    &false
                },
            )
            .field("local_sys_pubkey", &self.local_sys_pubkey)
            .field("creds", &creds_type)
            .finish()
    }
}

#[derive(Clone)]
pub struct CredPaths {
    host_creds_path: PathBuf,
    #[allow(dead_code)]
    sys_creds_path: Option<PathBuf>,
}

#[derive(Clone)]
pub enum AuthCredType {
    Guard(PathBuf),           // Default
    Authenticated(CredPaths), // Only assigned after successful hoster authentication
}

#[derive(Clone)]
pub struct Keys {
    host_keypair: KeyPair,
    pub host_pubkey: String,
    local_sys_keypair: Option<KeyPair>,
    pub local_sys_pubkey: Option<String>,
    pub creds: AuthCredType,
}

impl Keys {
    pub fn new() -> Result<Self> {
        let host_key_path = std::env::var("HOSTING_AGENT_HOST_NKEY_PATH")
            .context("Cannot read HOSTING_AGENT_HOST_NKEY_PATH from env var")?;
        let host_kp = KeyPair::new_user();
        write_keypair_to_file(PathBuf::from_str(&host_key_path)?, host_kp.clone())?;
        let host_pk = host_kp.public_key();

        let sys_key_path = std::env::var("HOSTING_AGENT_SYS_NKEY_PATH")
            .context("Cannot read SYS_NKEY_PATH from env var")?;
        let local_sys_kp = KeyPair::new_user();
        write_keypair_to_file(PathBuf::from_str(&sys_key_path)?, local_sys_kp.clone())?;
        let local_sys_pk = local_sys_kp.public_key();

        let auth_guard_creds =
            PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "AUTH", "auth_guard"))?;

        Ok(Self {
            host_keypair: host_kp,
            host_pubkey: host_pk,
            local_sys_keypair: Some(local_sys_kp),
            local_sys_pubkey: Some(local_sys_pk),
            creds: AuthCredType::Guard(auth_guard_creds),
        })
    }

    // NB: Only call when trying to load an already authenticated host user (with or without a sys user)
    pub fn try_from_storage(
        maybe_host_creds_path: &Option<PathBuf>,
        maybe_sys_creds_path: &Option<PathBuf>,
    ) -> Result<Self> {
        let host_key_path: String = std::env::var("HOSTING_AGENT_HOST_NKEY_PATH")
            .context("Cannot read HOSTING_AGENT_HOST_NKEY_PATH from env var")?;
        let host_keypair = try_read_keypair_from_file(PathBuf::from_str(&host_key_path.clone())?)?
            .ok_or_else(|| anyhow!("Host keypair not found at path {:?}", host_key_path))?;
        let host_pk = host_keypair.public_key();

        let auth_guard_creds =
            PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "AUTH", "auth_guard"))?;

        let host_user_name = format!("host_user_{}", host_pk);
        let host_creds_path = maybe_host_creds_path.to_owned().map_or_else(
            || PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "WORKLOAD", &host_user_name)),
            Ok,
        )?;

        let sys_user_name = format!("sys_user_{}", host_pk);
        let sys_creds_path = maybe_sys_creds_path.to_owned().map_or_else(
            || PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "SYS", &sys_user_name)),
            Ok,
        )?;

        let mut default_keys = Self {
            host_keypair,
            host_pubkey: host_pk,
            local_sys_keypair: None,
            local_sys_pubkey: None,
            creds: AuthCredType::Guard(auth_guard_creds), // Set auth_guard_creds as default user cred
        };

        let sys_key_path = std::env::var("HOSTING_AGENT_SYS_NKEY_PATH")
            .context("Cannot read HOSTING_AGENT_SYS_NKEY_PATH from env var")?;
        let keys = match try_read_keypair_from_file(PathBuf::from_str(&sys_key_path)?)? {
            Some(kp) => {
                let local_sys_pk = kp.public_key();
                default_keys.local_sys_keypair = Some(kp);
                default_keys.local_sys_pubkey = Some(local_sys_pk);
                default_keys
            }
            None => default_keys,
        };

        Ok(keys.clone().add_creds_paths(
            host_creds_path,
            Some(sys_creds_path)
        ).unwrap_or_else(move |e| {
            log::error!("Error: Cannot locate authenticated cred files. Defaulting to auth_guard_creds. Err={}",e);
            keys
        }))
    }

    pub fn _add_local_sys(mut self, sys_key_path: Option<PathBuf>) -> Result<Self> {
        let sys_key_path = sys_key_path.map_or_else(
            || PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "HPOS", "sys")),
            Ok,
        )?;

        let mut is_new_key = false;

        let local_sys_kp = try_read_keypair_from_file(sys_key_path.clone())?.unwrap_or_else(|| {
            is_new_key = true;
            KeyPair::new_user()
        });

        if is_new_key {
            write_keypair_to_file(sys_key_path, local_sys_kp.clone())?;
        }

        let local_sys_pk = local_sys_kp.public_key();

        self.local_sys_keypair = Some(local_sys_kp);
        self.local_sys_pubkey = Some(local_sys_pk);

        Ok(self)
    }

    pub fn add_creds_paths(
        mut self,
        host_creds_file_path: PathBuf,
        maybe_sys_creds_file_path: Option<PathBuf>,
    ) -> Result<Self> {
        match host_creds_file_path.try_exists() {
            Ok(is_ok) => {
                if !is_ok {
                    return Err(anyhow!(
                        "Failed to locate host creds path. Found broken sym link. Path={:?}",
                        host_creds_file_path
                    ));
                }

                let creds = match maybe_sys_creds_file_path {
                    Some(sys_path) => match sys_path.try_exists() {
                        Ok(is_ok) => {
                            if !is_ok {
                                return Err(anyhow!("Failed to locate sys creds path. Found broken sym link. Path={:?}", sys_path));
                            }
                            CredPaths {
                                host_creds_path: host_creds_file_path,
                                sys_creds_path: Some(sys_path),
                            }
                        }
                        Err(e) => {
                            return Err(anyhow!(
                                "Failed to locate sys creds path. Path={:?} Err={}",
                                sys_path,
                                e
                            ));
                        }
                    },
                    None => CredPaths {
                        host_creds_path: host_creds_file_path,
                        sys_creds_path: None,
                    },
                };
                self.creds = AuthCredType::Authenticated(creds);
                Ok(self)
            }
            Err(e) => Err(anyhow!(
                "Failed to locate host creds path. Path={:?} Err={}",
                host_creds_file_path,
                e
            )),
        }
    }

    pub async fn save_host_creds(
        &self,
        host_user_jwt: String,
        host_sys_user_jwt: String,
    ) -> Result<Self> {
        //  Save user jwt and sys jwt local to hosting agent
        let host_path = PathBuf::from_str(&format!(
            "{}/{}",
            get_local_creds_path(),
            "host.jwt"
        ))?;
        log::trace!("host_path={:?}", host_path);
        write_to_file(host_path.clone(), host_user_jwt.as_bytes())?;
        log::trace!("Wrote JWT to host file");

        let sys_path = PathBuf::from_str(&format!(
            "{}/{}",
            get_local_creds_path(),
            "sys.jwt"
        ))?;
        log::trace!("sys_path={:?}", sys_path);
        write_to_file(sys_path.clone(), host_sys_user_jwt.as_bytes())?;
        log::trace!("Wrote JWT to sys file");

        // Import host user jwt to local nsc resolver
        // TODO: Determine why the following works in cmd line, but doesn't seem to work when run in current program / run
        Command::new("nsc")
            .arg("import")
            .arg("user")
            .arg("--file")
            .arg(format!("{:?}", host_path))
            .output()
            .context("Failed to add import new host user on hosting agent.")?;
        log::trace!("Imported host user successfully");

        // Import sys user jwt to local nsc resolver
        Command::new("nsc")
            .arg("import")
            .arg("user")
            .arg("--file")
            .arg(format!("{:?}", sys_path))
            .output()
            .context("Failed to add import new sys user on hosting agent.")?;
        log::trace!("Imported sys user successfully");

        // Save user creds and sys creds local to hosting agent
        let host_user_name = format!("host_user_{}", self.host_pubkey);
        let host_creds_path =
            PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "WORKLOAD", &host_user_name))?;
        Command::new("nsc")
            .args([
                "generate",
                "creds",
                "--name",
                &host_user_name,
                "--account",
                "WORKLOAD",
                "--output-file",
                &host_creds_path.to_string_lossy(),
            ])
            .output()
            .context("Failed to add host user key to hosting agent")?;
        log::trace!(
            "Generated host user creds. creds_path={:?}",
            host_creds_path
        );

        let mut sys_creds_file_name = None;
        if self.local_sys_pubkey.as_ref().is_some() {
            let sys_user_name = format!("sys_user_{}", self.host_pubkey);
            let path = PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "SYS", &sys_user_name))?;
            Command::new("nsc")
                .args([
                    "generate",
                    "creds",
                    "--name",
                    &sys_user_name,
                    "--account",
                    "SYS",
                    "--output-file",
                    &path.to_string_lossy(),
                ])
                .output()
                .context("Failed to add sys user key to hosting agent")?;
            log::trace!("Generated sys user creds. creds_path={:?}", path);
            sys_creds_file_name = Some(path);
        }

        self.to_owned()
            .add_creds_paths(host_creds_path, sys_creds_file_name)
    }

    pub fn get_host_creds_path(&self) -> Option<PathBuf> {
        if let AuthCredType::Authenticated(creds) = self.to_owned().creds {
            return Some(creds.host_creds_path);
        };
        None
    }

    pub fn _get_sys_creds_path(&self) -> Option<PathBuf> {
        if let AuthCredType::Authenticated(creds) = self.to_owned().creds {
            return creds.sys_creds_path;
        };
        None
    }

    pub fn host_sign(&self, payload: &[u8]) -> Result<String> {
        let signature = self.host_keypair.sign(payload)?;

        Ok(BASE64URL_NOPAD.encode(&signature))
    }
}

fn write_keypair_to_file(key_file_path: PathBuf, keypair: KeyPair) -> Result<()> {
    let seed = keypair.seed()?;
    write_to_file(key_file_path, seed.as_bytes())
}

fn write_to_file(file_path: PathBuf, data: &[u8]) -> Result<()> {
    // TODO: ensure dirs already exist and create them if not...
    let mut file = File::create(&file_path)?;
    file.write_all(data)?;
    Ok(())
}

fn try_read_keypair_from_file(key_file_path: PathBuf) -> Result<Option<KeyPair>> {
    match try_read_from_file(key_file_path)? {
        Some(kps) => Ok(Some(KeyPair::from_seed(&kps)?)),
        None => Ok(None),
    }
}

fn try_read_from_file(file_path: PathBuf) -> Result<Option<String>> {
    match file_path.try_exists() {
        Ok(link_is_ok) => {
            if !link_is_ok {
                return Err(anyhow!(
                    "Failed to read path {:?}. Found broken sym link.",
                    file_path
                ));
            }

            let mut file_content = File::open(&file_path)
                .context(format!("Failed to open config file {:#?}", file_path))?;

            let mut s = String::new();
            file_content.read_to_string(&mut s)?;
            Ok(Some(s.trim().to_string()))
        }
        Err(_) => {
            log::debug!("No user file found at {:?}.", file_path);
            Ok(None)
        }
    }
}
