/*
Endpoints & Managed Subjects:
    - `update_workload`: handles the "WORKLOAD.<host_pukey>.update" subject
    - `fetch_workload_status`: handles the "WORKLOAD.<host_pukey>.send_status" subject
*/

use crate::types::WorkloadResult;

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use anyhow::{Context, Result};
use async_nats::{jetstream::kv::Store, Message};
use bson::oid::ObjectId;
use core::option::Option::None;
use db_utils::schemas::workload::{
    HappBinaryFormat, WorkloadManifest, WorkloadManifestHolochainDhtV1, WorkloadState,
    WorkloadStateDiscriminants, WorkloadStatePayload, WorkloadStatus,
};
use futures::TryFutureExt;
use ham::{
    exports::{
        holochain_client::AllowedOrigins, holochain_conductor_api::AppInterfaceInfo,
        holochain_types::app::AppBundle,
    },
    Ham,
};
use nats_utils::{macros::ApiOptions, types::ServiceError};
use serde::{Deserialize, Serialize};
use std::io::{Error as StdError, ErrorKind as StdErrorKind};
use std::path::PathBuf;
use std::{fmt::Debug, net::Ipv4Addr, path::Path, sync::Arc};
use url::Url;
use util::{
    bash, calculate_http_gw_port, ensure_workload_path, provision_extra_container_closure_path,
    realize_extra_container_path, EnsureWorkloadPathMode,
};

#[derive(Debug, Clone)]
pub struct HostWorkloadApi {
    // this is used as a persistence and communication layer to dynamically create handlers for the HTTP GW subjects
    pub hc_http_gw_storetore: Store,
}

impl WorkloadServiceApi for HostWorkloadApi {}

#[derive(thiserror::Error, Debug)]
#[error("error processing workload {workload_result:?}: {e}")]
struct WorkloadResultError {
    e: anyhow::Error,
    workload_result: WorkloadResult,
}

// TODO: create something that allocates ports and can be queried for a free one.
const HOLOCHAIN_ADMIN_PORT_DEFAULT: u16 = 8000;

#[derive(Deserialize)]
struct VersionConfig {
    supported_versions: Vec<String>,
}
impl VersionConfig {
    pub fn get_supported_versions() -> Self {
        let supported_hc_versions_static = VersionConfig {
            supported_versions: vec![
                "0.3".to_string(),
                "0.4".to_string(),
                "0.5".to_string(),
                "latest".to_string(),
            ],
        };

        let config_path = std::env::var("HOLOCHAIN_VERSION_CONFIG_PATH")
            .unwrap_or_else(|_| "../../supported-holochain-versions.json".to_string());

        std::fs::read_to_string(&config_path)
            .and_then(|content| {
                serde_json::from_str(&content)
                    .map_err(|e| StdError::new(StdErrorKind::InvalidData, e))
            })
            .unwrap_or_else(|e| {
                log::warn!(
                    "Failed to read or parse Holochain version config from '{}'. err={}. Using static default.",
                    config_path, e
                );
                supported_hc_versions_static
            })
    }
}

lazy_static::lazy_static! {
    static ref VERSION_CONFIG: VersionConfig = {
        VersionConfig::get_supported_versions()
    };
}

/// Validate if the requested Holochain version is supported
fn validate_holochain_version(version: Option<&String>) -> Result<(), String> {
    match version {
        Some(hc_version) => {
            let supported_versions = &VERSION_CONFIG.supported_versions;

            let parsed_version = hc_version.split('.').collect::<Vec<_>>();

            if parsed_version[0] == "latest" {
                return Ok(());
            } else if parsed_version.len() < 2 {
                return Err(format!("Invalid Holochain version format. Please use the format 'x.y' or 'x.y.z'. requested_version={}", hc_version));
            }
            let major_minor_version = format!("{}.{}", parsed_version[0], parsed_version[1]);
            if supported_versions.iter().any(|supported_version| {
                supported_version == hc_version || supported_version == &major_minor_version
            }) {
                Ok(())
            } else {
                Err(format!(
                    "Unsupported Holochain version '{}'. Supported versions are: {}. Please update your workload configuration to use a supported version.",
                    hc_version,
                    supported_versions.join(", ")
                ))
            }
        }
        None => Ok(()), // If no version is specified, this should fallback to default
    }
}

impl HostWorkloadApi {
    async fn handle_workload_command(
        &self,
        workload_result: WorkloadResult,
    ) -> anyhow::Result<WorkloadStatus> {
        match workload_result {
            WorkloadResult::Status(workload_status) => {
                log::warn!("Received a workload status message (WorkloadResult::Status). This is currently unsupported. Ignoring... ");
                Ok(workload_status)
            }
            WorkloadResult::Workload(workload) => {
                // Validate holochain version before attempting to install the holochain conductor and app bundle
                if let WorkloadManifest::HolochainDhtV1(ref inner) = workload.manifest {
                    if let Err(version_err) =
                        validate_holochain_version(inner.holochain_version.as_ref())
                    {
                        anyhow::bail!(
                            "Invalid holochain version provided in workload configuration. err={}",
                            version_err
                        );
                    }
                }

                // TODO(correctness): consider status.actual to inform assumptions towards the current state
                // TODO(backlog,ux): spawn longer-running tasks and report back Pending, and set up a periodic status updates while the spawned task is running
                let desired_state = &workload.status.desired;
                let (actual_state, workload_state_payload) = match desired_state {
                    WorkloadState::Running => {
                        let (workload_path_toplevel, _) = ensure_workload_path(
                            &workload._id,
                            None,
                            EnsureWorkloadPathMode::Create,
                        )?;
                        let extra_container_path = realize_extra_container_path(
                            workload._id,
                            workload.manifest.clone(),
                            PathBuf::from(&workload_path_toplevel),
                        )
                        .await?;

                        let start_or_restart_if_desired =
                            if let WorkloadState::Running = desired_state {
                                " --start --restart-changed"
                            } else {
                                ""
                            };

                        bash(&format!(
                            "extra-container create {extra_container_path}{start_or_restart_if_desired}",
                        ))
                        .await?;

                        let workload_state_payload = match (desired_state, &workload.manifest) {
                            (WorkloadState::Running, WorkloadManifest::HolochainDhtV1(boxed)) => {
                                let WorkloadManifestHolochainDhtV1 {
                                    happ_binary,
                                    network_seed,
                                    http_gw_enable,

                                    // acknowledge unused fields
                                    memproof: _,
                                    bootstrap_server_url: _,
                                    signal_server_url: _,
                                    stun_server_urls: _,
                                    holochain_feature_flags: _,
                                    holochain_version: _,
                                    http_gw_allowed_fns: _,
                                } = boxed.as_ref();

                                // TODO: wait until the container is considered booted
                                // TODO: wait until holochain is ready

                                let mut ham = Ham::connect(
                                    // TODO(feat): figure out the IP address of the container once we use network namespace separation
                                    Ipv4Addr::LOCALHOST,
                                    HOLOCHAIN_ADMIN_PORT_DEFAULT,
                                )
                                .await?;

                                let happ_binary_url = match happ_binary {
                                    HappBinaryFormat::HappBinaryUrl(url) => url,
                                    _ => anyhow::bail!(
                                        "Invalid happ binary format provided in workload configuration. Currently the only accepted format is the happ binary url. happ_binary_format={:?}",
                                        happ_binary
                                    ),
                                };

                                let happ_bytes = ham::Ham::download_happ_bytes(happ_binary_url)
                                    .await
                                    .context(format!("downloading {happ_binary_url:?}"))?;

                                // TODO(feat): derive a different predictable app id from the workload
                                let installed_app_id = workload._id.to_hex();

                                // TODO(clarify): how do we want to handle installing on a container that has previous state and the happ is already installed?
                                let (app_info, agent_key, app_interfaces) = if let Some(previous) =
                                    ham.find_installed_app(&installed_app_id).await?
                                {
                                    previous
                                } else {
                                    let (app_info, agent_key) = ham
                                        .install_and_enable_happ(
                                            &happ_bytes,
                                            network_seed.clone(),
                                            None,
                                            Some(installed_app_id.clone()),
                                        )
                                        .await?;

                                    let mut app_interface_info = AppInterfaceInfo {
                                        port: 0,
                                        // TODO(security)
                                        allowed_origins: AllowedOrigins::Any,
                                        installed_app_id: Some(installed_app_id.clone()),
                                    };

                                    let AppInterfaceInfo {
                                        port,
                                        allowed_origins,
                                        installed_app_id,
                                    } = &mut app_interface_info;

                                    // Connect app agent client
                                    // TODO(correctness): this port is going to be different after holochain (e.g. due to a machine reboot) is restarted
                                    *port = ham
                                        .admin_ws
                                        .attach_app_interface(
                                            *port,
                                            allowed_origins.clone(),
                                            installed_app_id.clone(),
                                        )
                                        .await
                                        .context("attaching app interface")?;

                                    (app_info, agent_key, vec![app_interface_info])
                                };

                                let app_ws_port = app_interfaces
                                    .first()
                                    .ok_or_else(|| anyhow::anyhow!("got no app interface"))?
                                    .port;

                                // TODO(clarify): iterate over all cells and call init until a call succeeds or cells are exhausted. treat errors as warnings.
                                let _happ =
                                    AppBundle::decode(&happ_bytes).context("decoding AppBundle")?;

                                let app_info_bson = bson::to_bson(&app_info)
                                    .context("serializing app_info into bson")?;

                                // persist the ham state for debugging purposes; maybe we can reuse it later
                                let _ = {
                                    let mut ham_state_builder = ham::HamStateBuilder::default();
                                    ham_state_builder = ham_state_builder.app_info(app_info);
                                    ham_state_builder = ham_state_builder.agent_key(agent_key);
                                    ham_state_builder = ham_state_builder.app_ws_port(app_ws_port);
                                    let ham_state =
                                        ham_state_builder.build().context("building HamState")?;
                                    let ham_state_path =
                                        Path::new(&workload_path_toplevel).join("ham.state");
                                    ham_state.persist(&ham_state_path).context(format!(
                                        "persisting ham state to {ham_state_path:?}"
                                    ))?;
                                    ham_state
                                };

                                if *http_gw_enable {
                                    /* When we have multiple workloads per host, we dynamically allocate/retrieve ip:port
                                        NB: We currently support multiple options:
                                        a) we use privateNetwork = true in containers and can use the default hc-http-gw port
                                        b) we use privateNetwork = false in containers and need different hc-http-gw ports
                                        option (b) is inherently less secure as the admin websockets will also be shared on the host network namespace
                                    */

                                    // // Set network configuration based on env var, defaulting to true.
                                    // let is_private_network =
                                    //     std::env::var("IS_CONTAINER_ON_PRIVATE_NETWORK")
                                    //         .unwrap_or_else(|_| "true".to_string())
                                    //         .parse::<bool>()
                                    //         .unwrap_or(true);
                                    // Set private netowrk flag based on env var, defaulting to false
                                    let is_private_network =
                                        std::env::var("IS_CONTAINER_ON_PRIVATE_NETWORK")
                                            .unwrap_or_else(|_| "false".to_string())
                                            .parse::<bool>()
                                            .unwrap_or(false);

                                    let hc_http_gw_port =
                                        calculate_http_gw_port(&workload._id, is_private_network);

                                    log::debug!("is_private_network: {is_private_network}");
                                    log::info!("hc_http_gw_port: {hc_http_gw_port}");

                                    let hc_http_gw_url_base = url::Url::parse(&format!(
                                        "http://127.0.0.1:{hc_http_gw_port}"
                                    ))?;

                                    match self
                                        .hc_http_gw_storetore
                                        .entry(workload._id.to_hex())
                                        .await
                                        .context("retrieving entry for {workload._id}")
                                    {
                                        Ok(Some(_)) => {
                                            // TODO: deal with this case smarter
                                            self.hc_http_gw_storetore
                                                .delete(workload._id.to_hex())
                                                .await
                                                .context("deleting entry for {workload._id}")?;
                                        }
                                        Ok(None) => {}
                                        Err(e) => {
                                            log::error!("{}", e);
                                        }
                                    }

                                    let key = workload._id.to_hex();
                                    let value = HcHttpGwWorkerKvBucketValue {
                                        desired_state: WorkloadStateDiscriminants::Running,
                                        hc_http_gw_url_base,
                                        installed_app_id,
                                    };
                                    let value_blob = serde_json::to_string(&value)
                                        .context(format!("serializing {value:?}"))?;

                                    self.hc_http_gw_storetore
                                        .create(&key, value_blob.into())
                                        .await
                                        .context(format!(
                                            "creating entry with key {key} and value {value:?}"
                                        ))?;
                                };

                                WorkloadStatePayload::HolochainDhtV1(app_info_bson)
                            }

                            _ => WorkloadStatePayload::None,
                        };

                        (desired_state, workload_state_payload)
                    }
                    WorkloadState::Uninstalled | WorkloadState::Deleted => {
                        let (workload_path_toplevel, path_exists) = ensure_workload_path(
                            &workload._id,
                            None,
                            EnsureWorkloadPathMode::Observe,
                        )?;

                        match workload.manifest {
                            WorkloadManifest::HolochainDhtV1(
                                ref workload_manifest_holochain_dht_v1,
                            ) if workload_manifest_holochain_dht_v1.http_gw_enable => {
                                let key = workload._id.to_hex();
                                if let Some(entry) = self
                                    .hc_http_gw_storetore
                                    .entry(&key)
                                    .await
                                    .context("retrieving entry for {workload._id}")
                                    .unwrap_or_else(|e| {
                                        log::error!("{e}");
                                        None
                                    })
                                {
                                    let desired_state: WorkloadStateDiscriminants =
                                        desired_state.into();
                                    log::debug!(
                                        "marking the KV entry for {key} to be become {desired_state:?}"
                                    );
                                    let mut value: HcHttpGwWorkerKvBucketValue =
                                        serde_json::from_slice(&entry.value).context(format!(
                                            "deserializing HcHttpGwWorkerKvBucketValue for {:?}",
                                            workload._id.to_hex()
                                        ))?;

                                    value.desired_state = desired_state;

                                    if let Err(e) = async {
                                        serde_json::to_string(&value).map_err(anyhow::Error::from)
                                    }
                                    .and_then(|serialized| {
                                        self.hc_http_gw_storetore
                                            .put(&key, serialized.into())
                                            .map_err(anyhow::Error::from)
                                    })
                                    .await
                                    {
                                        // best effort cleanup, don't throw here
                                        log::warn!("error putting the changed {key}: {e}");
                                    }
                                }
                            }

                            _ => (),
                        }

                        if path_exists {
                            let extra_container_path = provision_extra_container_closure_path(
                                &workload_path_toplevel.into(),
                            )?;

                            let extra_container_subcmd = match desired_state {
                                WorkloadState::Deleted => "destroy",
                                _ => "stop",
                            };

                            bash(&format!(
                                "extra-container {extra_container_subcmd} {extra_container_path}"
                            ))
                            .await?;

                            if Path::new(&extra_container_path).exists() {
                                log::debug!("removing container path at {extra_container_path}");
                                std::fs::remove_dir_all(&extra_container_path)
                                    .context(format!("removing {extra_container_path}"))?;
                            }
                        }

                        (desired_state, WorkloadStatePayload::None)
                    }

                    WorkloadState::Reported
                    | WorkloadState::Assigned
                    | WorkloadState::Updated
                    | WorkloadState::Pending
                    | WorkloadState::Error(_)
                    | WorkloadState::Unknown(_) => {
                        anyhow::bail!("unsupported desired state {desired_state:?}")
                    }
                };

                Ok(WorkloadStatus {
                    id: Some(workload._id),
                    desired: workload.status.desired.clone(),
                    actual: actual_state.clone(),
                    payload: workload_state_payload,
                })
            }
        }
    }

    pub async fn update_workload(
        &self,
        msg: Arc<Message>,
        api_options: ApiOptions,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let msg_subject = msg.subject.clone().into_string();
        let msg_headers = msg.headers.clone();
        log::trace!("Incoming message for '{}'", msg_subject);

        let mut header_map = async_nats::HeaderMap::new();
        let host_device_id = api_options.device_id;
        header_map.insert("host_id", host_device_id);

        // TODO: fix -
        // Note: Throwing an actual error in this scope leads to the request silently skipped with no logs entry in the host-agent.
        let workload_payload = match Self::convert_msg_to_type::<WorkloadResult>(msg) {
            Ok(r) => r,
            Err(err) => {
                return Ok(WorkloadApiResult {
                    maybe_response_tags: None,
                    result: WorkloadResult::Status(Self::handle_error(
                        err.into(),
                        msg_subject,
                        msg_headers,
                    )),
                    maybe_headers: Some(header_map),
                });
            }
        };

        log::debug!(
            "Received update workload message. subject={msg_subject}', msg={workload_payload:?}"
        );

        let workload_status = match self.handle_workload_command(workload_payload).await {
            Ok(res) => res,
            Err(err) => {
                let workload_status = Self::handle_error(err, msg_subject, msg_headers);
                log::error!("{workload_status:?}");
                workload_status
            }
        };

        Ok(WorkloadApiResult {
            maybe_response_tags: None,
            result: WorkloadResult::Status(workload_status),
            maybe_headers: Some(header_map),
        })
    }

    pub async fn fetch_workload_status(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let workload_payload = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        let current_status = match workload_payload {
            WorkloadResult::Workload(workload) => {
                // let last_recorded_status = workload.status;

                // TODO: this is a placeholder for fetching the actual status on the system for this workload
                // we're interested in the actual status on the system for this workload

                // TODO: look up the status for the given workload

                workload.status
            }
            WorkloadResult::Status(status) => {
                log::warn!("Received a workload status message (WorkloadResult::Status). This is currently unsupported. Ignoring... ");
                status
            }
        };

        // Send updated status:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(WorkloadApiResult {
            result: WorkloadResult::Status(current_status),
            maybe_response_tags: None,
            maybe_headers: None,
        })
    }

    fn handle_error(
        err: anyhow::Error,
        msg_subject: String,
        maybe_msg_headers: Option<async_nats::HeaderMap>,
    ) -> WorkloadStatus {
        match err.downcast::<WorkloadResultError>() {
            Ok(WorkloadResultError {
                e,
                workload_result: payload,
            }) => match payload {
                WorkloadResult::Workload(workload) => WorkloadStatus {
                    id: Some(workload._id),
                    actual: WorkloadState::Error(e.to_string()),
                    ..workload.status
                },
                WorkloadResult::Status(s) => s,
            },
            Err(e) => {
                let workload_id = maybe_msg_headers.and_then(|header_map| {
                    header_map
                        .get("workload_id")
                        .and_then(|workload_id| ObjectId::parse_str(workload_id).ok())
                });

                WorkloadStatus {
                    id: workload_id,
                    desired: WorkloadState::Unknown(Default::default()),
                    actual: WorkloadState::Error(format!(
                        "Error handling workload update. request_subject={msg_subject}, err={e:?}"
                    )),
                    payload: Default::default(),
                }
            }
        }
    }
}

mod util {
    use crate::host_api::{
        get_container_name, validate_holochain_version, HOLOCHAIN_ADMIN_PORT_DEFAULT,
    };
    use anyhow::Context;
    use bson::oid::ObjectId;
    use db_utils::schemas::workload::{WorkloadManifest, WorkloadManifestHolochainDhtV1};
    use futures::{AsyncBufReadExt, StreamExt};
    use sha2::{Digest, Sha256};
    use std::{path::PathBuf, process::Stdio, str::FromStr};
    use tokio::process::Command;

    /// Calculate the http gw port based on network mode and workload ID
    ///
    /// With privateNetwork=true, we can use the standard port inside containers
    /// and let nixos handle port forwarding to avoid conflicts
    /// With shared network, use dynamic port allocation based on workload ID
    ///
    /// For holochain gateway access from outside the container, we should use the dynamically allocated host port
    /// (This will be the same as the container port when privateNetwork=true with port forwarding)
    pub(crate) fn calculate_http_gw_port(
        workload_id: &bson::oid::ObjectId,
        is_private_network: bool,
    ) -> u16 {
        const HOLOCHAIN_HTTP_GW_PORT_DEFAULT: u16 = 8090;
        const PORT_RANGE: u16 = 10000;
        if is_private_network {
            // With private networks, use standard port (forwarded to host)
            HOLOCHAIN_HTTP_GW_PORT_DEFAULT
        } else {
            // With shared network, use dynamic port allocation based on workload ID
            // Create deterministic port offset from workload id (same as in extra-container-holochain nix package)
            let hex = workload_id.to_hex();
            let mut hasher = Sha256::new();
            hasher.update(hex.as_bytes());
            let hash = hasher.finalize();
            let offset = u16::from_le_bytes([hash[0], hash[1]]) % PORT_RANGE;
            HOLOCHAIN_HTTP_GW_PORT_DEFAULT + offset
        }
    }

    pub(crate) async fn bash(cmd: &str) -> anyhow::Result<()> {
        let mut workload_cmd = tokio::process::Command::new("/usr/bin/env");
        workload_cmd.args(["bash", "-c", cmd]);

        log::trace!("running bash command: {cmd}");

        let output = workload_cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("spawning {cmd}"))?
            .wait_with_output()
            .await
            .context(format!("waiting for spawned command: {cmd}"))?;

        if !output.status.success() {
            anyhow::bail!("error running {workload_cmd:?} yielded non-success status:\n{output:?}");
        }

        log::info!("workload creation result:\n{output:#?}");

        Ok(())
    }

    pub(crate) enum EnsureWorkloadPathMode {
        Create,
        // Exists,
        Observe,
    }

    pub(crate) fn ensure_workload_path(
        id: &ObjectId,
        maybe_subdir: Option<&str>,
        mode: EnsureWorkloadPathMode,
    ) -> anyhow::Result<(String, bool)> {
        const WORKLOAD_BASE_PATH: &str = "/var/lib/holo-host-agent/workloads";

        let workload_path = {
            let dir = PathBuf::from_str(WORKLOAD_BASE_PATH)
                .map(|p| p.join(id.to_hex()))
                .context(format!("parsing {WORKLOAD_BASE_PATH} to PathBuf"))?;

            if let Some(subdir) = maybe_subdir {
                dir.join(subdir)
            } else {
                dir
            }
        };

        let path_exists = match mode {
            EnsureWorkloadPathMode::Create => {
                std::fs::create_dir_all(&workload_path).map(|()| true)?
            }
            EnsureWorkloadPathMode::Observe => std::fs::exists(&workload_path)?,
        };

        let path = workload_path.to_str().map(ToString::to_string).ok_or_else(|| anyhow::anyhow!("{workload_path:?} is not a valid string, and we need to use it in string representation"))?;

        Ok((path, path_exists))
    }

    // Transform the workload into something that can be executed
    pub(crate) async fn realize_extra_container_path(
        workload_id: ObjectId,
        manifest: WorkloadManifest,
        workload_path: PathBuf,
    ) -> anyhow::Result<String> {
        log::debug!("transforming {manifest:?} at {workload_path:?}");

        match manifest {
            WorkloadManifest::None => {
                anyhow::bail!("cannot install anything without a manifest");
            }
            WorkloadManifest::ExtraContainerBuildCmd { nix_args } => {
                let output = {
                    let mut tokio_cmd = Command::new("/usr/bin/env");
                    tokio_cmd.args(["nix", "build", "--no-link", "--print-out-paths"]);
                    tokio_cmd.args(nix_args);

                    let msg = format!("spawning build command: {tokio_cmd:?}");
                    log::debug!("{}", msg);
                    tokio_cmd
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                        .context(msg.clone())?
                        .wait_with_output()
                        .await
                        .context(msg)?
                };

                if !output.status.success() {
                    anyhow::bail!("error running command: {output:?}");
                }

                let path = output
                    .stdout
                    .lines()
                    .next()
                    .await
                    .transpose()?
                    .ok_or_else(|| {
                        anyhow::anyhow!("couldn't get first line from output {output:?}")
                    })?;

                if !path.starts_with("/nix/store") {
                    anyhow::bail!("not a /nix/store path: {path}");
                }

                Box::pin(realize_extra_container_path(
                    workload_id,
                    WorkloadManifest::ExtraContainerStorePath {
                        store_path: path.into(),
                    },
                    workload_path,
                ))
                .await
            }

            WorkloadManifest::ExtraContainerStorePath { store_path } => {
                let container_closure_path =
                    provision_extra_container_closure_path(&workload_path)?;

                // use a transient name for the symlink to achieve an atomic operation
                let symlink_transient_name = format!("{container_closure_path}.new");
                std::os::unix::fs::symlink(&store_path, &symlink_transient_name).context(
                    format!(
                        "linking workload_path from {store_path:?} to {container_closure_path:?}"
                    ),
                )?;
                std::fs::rename(&symlink_transient_name, &container_closure_path).context(
                    format!("renaming {symlink_transient_name} to {container_closure_path}"),
                )?;

                Box::pin(realize_extra_container_path(
                    workload_id,
                    WorkloadManifest::ExtraContainerPath {
                        extra_container_path: container_closure_path,
                    },
                    workload_path,
                ))
                .await
            }

            WorkloadManifest::ExtraContainerPath {
                extra_container_path,
            } => Ok(extra_container_path),

            WorkloadManifest::HolochainDhtV1(inner) => {
                let WorkloadManifestHolochainDhtV1 {
                    bootstrap_server_url,
                    signal_server_url,
                    holochain_feature_flags,
                    stun_server_urls,
                    http_gw_enable,
                    http_gw_allowed_fns,
                    holochain_version,

                    // not relevant here
                    happ_binary: _,
                    network_seed: _,
                    memproof: _,
                } = *inner;

                // Validate the holochain version before proceeding
                if let Err(validation_error) =
                    validate_holochain_version(holochain_version.as_ref())
                {
                    anyhow::bail!("Holochain version validation failed: {}", validation_error);
                }

                // Set private netowrk flag based on env var, defaulting to false
                let is_private_network = std::env::var("IS_CONTAINER_ON_PRIVATE_NETWORK")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse::<bool>()
                    .unwrap_or(false);

                // This is used to store the key=value pairs for the attrset that is passed to `.override attrs`
                let mut override_attrs = vec![
                    format!(r#"containerName = "{}""#, get_container_name(&workload_id)?),
                    format!(r#"workloadId = "{}""#, workload_id.to_hex()),
                    format!(r#"adminWebsocketPort = {}"#, HOLOCHAIN_ADMIN_PORT_DEFAULT),
                    format!(r#"httpGwEnable = {}"#, http_gw_enable),
                    // NB: We use this setting for both container networking and port allocation defaults
                    format!(r#"privateNetwork = {}"#, is_private_network),
                    // TODO: clarify if we want to autostart the container unconditionally
                    format!(r#"autoStart = true"#),
                ];

                // If we're not on a private network, we need to set the http gw port to a dynamic value based on workload id,
                // otherwise, the port will be set to the default value (8090) automatically at container build time
                if !is_private_network && http_gw_enable {
                    let hc_http_gw_port = calculate_http_gw_port(&workload_id, is_private_network);
                    override_attrs.push(format!(r#"httpGwPort = {}"#, hc_http_gw_port));
                }

                if let Some(url) = bootstrap_server_url {
                    override_attrs.push(format!(r#"bootstrapUrl = "{url}""#));
                }

                if let Some(url) = signal_server_url {
                    override_attrs.push(format!(r#"signalUrl = "{url}""#));
                }

                if let Some(urls) = stun_server_urls {
                    override_attrs.push(format!(
                        r#"stunUrls = [{}]"#,
                        urls.iter()
                            .map(|url| format!(r#""{url}""#))
                            .collect::<Vec<String>>()
                            .join(" "),
                    ));
                }

                if let Some(flags) = holochain_feature_flags {
                    override_attrs.push(format!(
                        r#"holochainFeatures = [{}]"#,
                        flags
                            .iter()
                            .map(|flag| format!(r#""{flag}""#))
                            .collect::<Vec<String>>()
                            .join(" "),
                    ));
                }

                if let Some(version) = holochain_version {
                    override_attrs.push(format!(r#"holochainVersion = "{}""#, version));
                }

                if http_gw_enable {
                    // reminder: we pass the the workload_id as the installed_app_id at app install time
                    // eventually these may more more than one
                    let list_stringified = &[workload_id]
                        .iter()
                        .map(|s| format!(r#""{s}""#))
                        .collect::<Vec<_>>()
                        .join(" ");

                    override_attrs.push(format!("httpGwAllowedAppIds = [{list_stringified}]"));

                    if let Some(_allowed_fns) = http_gw_allowed_fns {
                        // TODO(security)
                        /* produce an attrset like this
                        ```nix
                        {
                            ${appId} = [
                                ${allowedFn0}
                                ${allowedFn1}
                            ];
                        }
                        ```
                        */
                    }
                }

                let override_attrs_stringified = override_attrs.join("; ") + ";";

                log::debug!(
                    "passing the these overrides to holochain: '{override_attrs_stringified}'"
                );

                let flake_url = std::env::var("HOLO_HOST_FLAKE_URL")
                    .unwrap_or_else(|_| "github:holo-host/holo-host/db-streaming".to_string());

                let nix_build_args = [
                    "--refresh",
                    "--extra-experimental-features",
                    "nix-command flakes",
                    "--impure",
                    "--expr",
                    &format!(
                        // TODO(feat): make this configurable and use something more dynamic.
                        r#"(builtins.getFlake "{flake_url}").packages.${{builtins.currentSystem}}.extra-container-holochain.override {{ {override_attrs_stringified} }}"#
                    ),
                ]
                .into_iter()
                .map(ToString::to_string)
                .collect();

                Box::pin(realize_extra_container_path(
                    workload_id,
                    WorkloadManifest::ExtraContainerBuildCmd {
                        nix_args: nix_build_args,
                    },
                    workload_path,
                ))
                .await
            }
        }
    }

    pub(crate) fn provision_extra_container_closure_path(
        workload_path: &PathBuf,
    ) -> anyhow::Result<String> {
        let joined = [
            workload_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("cannot convert {workload_path:?} to string"))?,
            "extra-container",
        ]
        .join("/");

        Ok(joined)
    }
}

// TODO: as this is the hostname we're limited to 11 characters. make sure it's unique
fn get_container_name(workload_id: &ObjectId) -> anyhow::Result<String> {
    const MIN_LENGTH: usize = 11;
    if workload_id.to_hex().len() >= MIN_LENGTH {
        Ok(workload_id.to_hex()[0..(MIN_LENGTH - 1)].to_string())
    } else {
        anyhow::bail!("{workload_id} needs a minimum hex length of {MIN_LENGTH}");
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HcHttpGwWorkerKvBucketValue {
    pub desired_state: WorkloadStateDiscriminants,
    pub hc_http_gw_url_base: Url,
    pub installed_app_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_supported_holochain_versions() {
        let versions = &VERSION_CONFIG.supported_versions;
        assert!(versions.contains(&"0.4".to_string()));
        assert!(versions.contains(&"latest".to_string()));
        assert!(!versions.is_empty());
    }

    #[test]
    fn test_validate_holochain_version_supported() {
        // Test supported versions
        assert!(validate_holochain_version(Some(&"0.4".to_string())).is_ok());
        assert!(validate_holochain_version(Some(&"0.3".to_string())).is_ok());
        assert!(validate_holochain_version(Some(&"latest".to_string())).is_ok());
        assert!(validate_holochain_version(Some(&"0.4.0".to_string())).is_ok());

        // Test None (should be okay)
        assert!(validate_holochain_version(None).is_ok());
    }

    #[test]
    fn test_validate_holochain_version_unsupported() {
        // Test unsupported versions
        let result = validate_holochain_version(Some(&"0.2".to_string()));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Unsupported Holochain version '0.2'"));

        let result = validate_holochain_version(Some(&"1.0".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Supported versions are:"));

        let result = validate_holochain_version(Some(&"invalid".to_string()));
        assert!(result.is_err());
    }
}
