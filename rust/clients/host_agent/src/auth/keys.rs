use data_encoding::BASE64URL_NOPAD;
use nats_utils::jetstream_client::{get_local_creds_path, get_nats_creds_by_nsc};
use nkeys::KeyPair;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;

use crate::local_cmds::host::errors::{HostAgentError, HostAgentResult};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

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
    pub host_creds_path: PathBuf,
    #[allow(dead_code)]
    pub sys_creds_path: Option<PathBuf>,
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
    pub fn new() -> HostAgentResult<Self> {
        let host_key_path = std::env::var("HOSTING_AGENT_HOST_NKEY_PATH")?;
        let host_kp = KeyPair::new_user();
        write_keypair_to_file(PathBuf::from_str(&host_key_path)?, host_kp.clone())?;
        let host_pk = host_kp.public_key();

        let sys_key_path = std::env::var("HOSTING_AGENT_SYS_NKEY_PATH")?;
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
    ) -> HostAgentResult<Self> {
        let host_key_path = std::env::var("HOSTING_AGENT_HOST_NKEY_PATH")?;
        let host_keypair = try_read_keypair_from_file(PathBuf::from_str(&host_key_path)?)?
            .ok_or_else(|| {
                HostAgentError::service_failed(
                    "host keypair loading",
                    &format!("Host keypair not found at path {:?}", host_key_path),
                )
            })?;
        let host_pk = host_keypair.public_key();

        let auth_guard_creds =
            PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "AUTH", "auth_guard"))?;

        let host_user_name = format!("host_user_{}", host_pk);
        let host_creds_path = maybe_host_creds_path.to_owned().map_or_else(
            || PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "HPOS", &host_user_name)),
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

        let sys_key_path = std::env::var("HOSTING_AGENT_SYS_NKEY_PATH")?;
        let keys = match try_read_keypair_from_file(PathBuf::from_str(&sys_key_path)?)? {
            Some(kp) => {
                let local_sys_pk = kp.public_key();
                default_keys.local_sys_keypair = Some(kp);
                default_keys.local_sys_pubkey = Some(local_sys_pk);
                default_keys
            }
            None => default_keys,
        };

        let keys_clone = keys.clone();
        match keys_clone.add_creds_paths(host_creds_path, Some(sys_creds_path)) {
            Ok(authenticated_keys) => Ok(authenticated_keys),
            Err(e) => {
                log::error!("Cannot locate authenticated cred files. Defaulting to auth_guard_creds. Err={}", e);
                Ok(keys)
            }
        }
    }

    pub fn _add_local_sys(mut self, sys_key_path: Option<PathBuf>) -> HostAgentResult<Self> {
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
    ) -> HostAgentResult<Self> {
        match host_creds_file_path.try_exists() {
            Ok(true) => {
                let creds = match maybe_sys_creds_file_path {
                    Some(sys_path) => {
                        match sys_path.try_exists() {
                            Ok(true) => CredPaths {
                                host_creds_path: host_creds_file_path,
                                sys_creds_path: Some(sys_path),
                            },
                            Ok(false) => {
                                return Err(HostAgentError::service_failed(
                                    "sys creds validation",
                                    &format!("Sys creds file does not exist. Path={:?}", sys_path),
                                ));
                            }
                            Err(e) => {
                                return Err(HostAgentError::service_failed("sys creds validation", &format!("Failed to check sys creds path existence. Path={:?} Err={}", sys_path, e)));
                            }
                        }
                    }
                    None => CredPaths {
                        host_creds_path: host_creds_file_path,
                        sys_creds_path: None,
                    },
                };
                self.creds = AuthCredType::Authenticated(creds);
                Ok(self)
            }
            Ok(false) => Err(HostAgentError::service_failed(
                "host creds validation",
                &format!(
                    "Host creds file does not exist. Path={:?}",
                    host_creds_file_path
                ),
            )),
            Err(e) => Err(HostAgentError::service_failed(
                "host creds validation",
                &format!(
                    "Failed to check host creds path existence. Path={:?} Err={}",
                    host_creds_file_path, e
                ),
            )),
        }
    }

    pub async fn save_host_creds(
        &self,
        host_user_jwt: String,
        host_sys_user_jwt: String,
    ) -> HostAgentResult<Self> {
        let local_creds_path = PathBuf::from(get_local_creds_path());
        create_dir_all(&local_creds_path)?;

        let host_path = local_creds_path.join("host.jwt");
        let sys_path = local_creds_path.join("sys.jwt");

        let mut created_files = Vec::new();

        log::trace!("host_path={:?}", host_path);
        write_to_file(host_path.clone(), host_user_jwt.as_bytes())?;
        created_files.push(host_path.clone());
        log::trace!("Wrote JWT to host file");

        log::trace!("sys_path={:?}", sys_path);
        if let Err(e) = write_to_file(sys_path.clone(), host_sys_user_jwt.as_bytes()) {
            cleanup_files(&created_files);
            return Err(HostAgentError::service_failed(
                "sys JWT write",
                &format!("Failed to write sys JWT to {:?}: {}", sys_path, e),
            ));
        }
        created_files.push(sys_path.clone());
        log::trace!("Wrote JWT to sys file");

        // Import host user jwt to local nsc resolver
        // TODO: Determine why the following works in cmd line, but doesn't seem to work when run in current program / run
        if let Err(e) = Command::new("nsc")
            .arg("import")
            .arg("user")
            .arg("--file")
            .arg(host_path.to_string_lossy().as_ref())
            .output()
        {
            // Cleanup on failure
            cleanup_files(&created_files);
            return Err(HostAgentError::from(e));
        }
        log::trace!("Imported host user successfully");

        // Import sys user jwt to local nsc resolver
        if let Err(e) = Command::new("nsc")
            .arg("import")
            .arg("user")
            .arg("--file")
            .arg(sys_path.to_string_lossy().as_ref())
            .output()
        {
            // Cleanup on failure
            cleanup_files(&created_files);
            return Err(HostAgentError::from(e));
        }
        log::trace!("Imported sys user successfully");

        let host_user_name = format!("host_user_{}", self.host_pubkey);
        let host_creds_path =
            PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "HPOS", &host_user_name))?;

        if let Err(e) = execute_nsc_command(
            &[
                "generate",
                "creds",
                "--name",
                &host_user_name,
                "--account",
                "HPOS",
                "--output-file",
                host_creds_path.to_string_lossy().as_ref(),
            ],
            "Failed to generate host user credentials",
        ) {
            cleanup_files(&created_files);
            return Err(e);
        }
        created_files.push(host_creds_path.clone());
        log::trace!(
            "Generated host user creds. creds_path={:?}",
            host_creds_path
        );

        let mut sys_creds_file_name = None;
        if self.local_sys_pubkey.as_ref().is_some() {
            let sys_user_name = format!("sys_user_{}", self.host_pubkey);
            let path = PathBuf::from_str(&get_nats_creds_by_nsc("HOLO", "SYS", &sys_user_name))?;

            if let Err(e) = execute_nsc_command(
                &[
                    "generate",
                    "creds",
                    "--name",
                    &sys_user_name,
                    "--account",
                    "SYS",
                    "--output-file",
                    path.to_string_lossy().as_ref(),
                ],
                "Failed to generate sys user credentials",
            ) {
                cleanup_files(&created_files);
                return Err(e);
            }
            log::trace!("Generated sys user creds. creds_path={:?}", path);
            sys_creds_file_name = Some(path);
        }

        self.clone()
            .add_creds_paths(host_creds_path, sys_creds_file_name)
    }

    pub fn get_host_creds_path(&self) -> Option<&PathBuf> {
        match &self.creds {
            AuthCredType::Authenticated(creds) => Some(&creds.host_creds_path),
            AuthCredType::Guard(_) => None,
        }
    }

    pub fn _get_sys_creds_path(&self) -> Option<&PathBuf> {
        match &self.creds {
            AuthCredType::Authenticated(creds) => creds.sys_creds_path.as_ref(),
            AuthCredType::Guard(_) => None,
        }
    }

    pub fn host_sign(&self, payload: &[u8]) -> HostAgentResult<String> {
        let signature = self.host_keypair.sign(payload)?;

        Ok(BASE64URL_NOPAD.encode(&signature))
    }
}

fn execute_nsc_command(args: &[&str], error_msg: &str) -> HostAgentResult<()> {
    log::debug!("Executing nsc command: nsc {}", args.join(" "));

    let output = Command::new("nsc").args(args).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(HostAgentError::service_failed(
            "nsc command",
            &format!(
                "{}: exit code {}\nstdout: {}\nstderr: {}",
                error_msg,
                output.status.code().unwrap_or(-1),
                stdout,
                stderr
            ),
        ));
    }

    log::debug!("nsc command completed successfully");
    Ok(())
}

fn cleanup_files(files: &[PathBuf]) {
    for file in files {
        if let Err(e) = std::fs::remove_file(file) {
            log::warn!("Failed to cleanup file {:?}: {}", file, e);
        } else {
            log::debug!("Cleaned up file: {:?}", file);
        }
    }
}

fn write_keypair_to_file(key_file_path: PathBuf, keypair: KeyPair) -> HostAgentResult<()> {
    let seed = keypair.seed()?;
    write_to_file(key_file_path, seed.as_bytes())
}

// Writes data to a file with secure permissions (readable only by owner)
fn write_to_file(file_path: PathBuf, data: &[u8]) -> HostAgentResult<()> {
    // Ensure parent directories exist
    if let Some(parent) = file_path.parent() {
        create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        // Create file with restrictive permissions (600 - read/write for owner only)
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&file_path)?;

        file.write_all(data)?;
    }

    #[cfg(not(unix))]
    {
        // Fallback for non-Unix systems
        let mut file = File::create(&file_path)?;
        file.write_all(data)?;
    }

    Ok(())
}

fn try_read_keypair_from_file(key_file_path: PathBuf) -> HostAgentResult<Option<KeyPair>> {
    match try_read_from_file(key_file_path)? {
        Some(kps) => Ok(Some(KeyPair::from_seed(&kps)?)),
        None => Ok(None),
    }
}

fn try_read_from_file(file_path: PathBuf) -> HostAgentResult<Option<String>> {
    match file_path.try_exists() {
        Ok(true) => {
            // File exists, try to read it
            let mut file_content = File::open(&file_path)?;

            let mut s = String::new();
            file_content.read_to_string(&mut s)?;

            Ok(Some(s.trim().to_string()))
        }
        Ok(false) => {
            // File doesn't exist (including broken symlinks)
            log::debug!("No file found at {:?}", file_path);
            Ok(None)
        }
        Err(e) => {
            // Permission denied or other I/O error
            Err(HostAgentError::service_failed(
                "file existence check",
                &format!("Failed to check file existence at {:?}: {}", file_path, e),
            ))
        }
    }
}
