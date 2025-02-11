/*
Endpoints & Managed Subjects:
    - `install_workload`: handles the "WORKLOAD.<host_pukey>.install." subject
    - `update_workload`: handles the "WORKLOAD.<host_pukey>.update_installed" subject
    - `uninstall_workload`: handles the "WORKLOAD.<host_pukey>.uninstall." subject
    - `fetch_workload_status`: handles the "WORKLOAD.<host_pukey>.send_status" subject
*/

use crate::types::WorkloadResult;

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use anyhow::{Context, Result};
use async_nats::Message;
use bson::oid::ObjectId;
use core::option::Option::None;
use db_utils::schemas::{
    Workload, WorkloadManifest, WorkloadManifestHolochainDhtV1, WorkloadState,
    WorkloadStatePayload, WorkloadStatus,
};
use ham::{
    exports::{
        holochain_client::AllowedOrigins, holochain_conductor_api::AppInterfaceInfo,
        holochain_types::app::AppBundle,
    },
    Ham,
};
use nats_utils::types::ServiceError;
use std::{fmt::Debug, net::Ipv4Addr, path::Path, sync::Arc};
use util::{
    bash, ensure_workload_path, get_workload_id, provision_extra_container_closure_path,
    realize_extra_container_path,
};

#[derive(Debug, Clone, Default)]
pub struct HostWorkloadApi {}

impl WorkloadServiceApi for HostWorkloadApi {}

#[derive(thiserror::Error, Debug)]
#[error("error processing workload {workload_result:?}: {e}")]
struct WorkloadResultError {
    e: anyhow::Error,
    workload_result: WorkloadResult,
}

// TODO: create something that allocates ports and can be queried for a free one.
const HOLOCHAIN_ADMIN_PORT_DEFAULT: u16 = 8000;

impl HostWorkloadApi {
    async fn handle_workload_command(
        msg_subject: String,
        try_message_payload: Result<WorkloadResult, ServiceError>,
    ) -> anyhow::Result<(WorkloadStatus, Workload)> {
        let workload_result = try_message_payload?;

        let workload_id = get_workload_id(&workload_result).map_err(|e| WorkloadResultError {
            e,
            workload_result: workload_result.clone(),
        })?;

        let WorkloadResult {
            workload: maybe_workload,
            ..
        } = workload_result;

        let workload = match maybe_workload {
            Some(workload) => workload,
            None => anyhow::bail!("Failed to process Workload Service Endpoint. Subject={} Error=No workload found in message.", msg_subject),
        };

        // TODO(correctness): consider status.actual to inform assumptions towards the current state
        // TODO(backlog,ux): spawn longer-running tasks and report back Pending, and set up a periodic status updates while the spawned task is running

        let desired_state = &workload.status.desired;
        let (actual_status, status_payload) = match desired_state {
            WorkloadState::Installed | WorkloadState::Running => {
                let (workload_path_toplevel, _) =
                    ensure_workload_path(&workload_id, None, util::EnsureWorkloadPathMode::Create)?;
                let extra_container_path = realize_extra_container_path(
                    workload_id,
                    workload.manifest.clone(),
                    (&workload_path_toplevel).into(),
                )
                .await?;

                let start_or_restart_if_desired = if let WorkloadState::Running = desired_state {
                    " --start --restart-changed"
                } else {
                    ""
                };

                bash(&format!(
                    "extra-container create {extra_container_path}{start_or_restart_if_desired}",
                ))
                .await?;

                let status_payload = match (desired_state, &workload.manifest) {
                    (WorkloadState::Running, WorkloadManifest::HolochainDhtV1(boxed)) => {
                        let WorkloadManifestHolochainDhtV1 {
                            happ_binary_url,
                            network_seed,
                            ..
                        } = boxed.as_ref();

                        // TODO: wait until the container is considered booted
                        // TODO: wait until holochain is ready

                        let mut ham = Ham::connect(
                            // TODO(feat): figure out the IP address of the container once we use network namespace separation
                            Ipv4Addr::LOCALHOST,
                            HOLOCHAIN_ADMIN_PORT_DEFAULT,
                        )
                        .await?;

                        let happ_bytes = ham::Ham::download_happ_bytes(happ_binary_url)
                            .await
                            .context(format!("downloading {happ_binary_url:?}"))?;

                        // TODO(feat): derive a different predictable app id from the workload
                        let installed_app_id = workload_id.to_hex();

                        // TODO(clarify): how do we want to handle installing on a container that has previous state and the happ is already installed?
                        let (app_info, agent_key, app_interfaces) = if let Some(previous) =
                            ham.find_installed_app(&installed_app_id).await?
                        {
                            previous
                        } else {
                            let (app_info, agent_key) = ham
                                .install_and_enable_happ(
                                    &happ_bytes,
                                    Some(network_seed.to_string()),
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
                        let _happ = AppBundle::decode(&happ_bytes).context("decoding AppBundle")?;

                        let app_info_bson =
                            bson::to_bson(&app_info).context("serializing app_info into bson")?;

                        // persist the ham state for debugging purposes; maybe we can reuse it later
                        {
                            let mut ham_state_builder = ham::HamStateBuilder::default();
                            ham_state_builder = ham_state_builder.app_info(app_info);
                            ham_state_builder = ham_state_builder.agent_key(agent_key);
                            ham_state_builder = ham_state_builder.app_ws_port(app_ws_port);
                            let ham_state =
                                ham_state_builder.build().context("building HamState")?;
                            let ham_state_path =
                                Path::new(&workload_path_toplevel).join("ham.state");
                            ham_state
                                .persist(&ham_state_path)
                                .context(format!("persisting ham state to {ham_state_path:?}"))?;
                        }

                        db_utils::schemas::WorkloadStatePayload::HolochainDhtV1(app_info_bson)
                    }

                    _ => WorkloadStatePayload::None,
                };

                (desired_state, status_payload)
            }
            WorkloadState::Uninstalled | WorkloadState::Removed | WorkloadState::Deleted => {
                let (workload_path_toplevel, exists) = ensure_workload_path(
                    &workload_id,
                    None,
                    util::EnsureWorkloadPathMode::Observe,
                )?;

                if exists {
                    let extra_container_path =
                        provision_extra_container_closure_path(&workload_path_toplevel.into())?;

                    bash(&format!("extra-container destroy {extra_container_path}")).await?;

                    if Path::new(&extra_container_path).exists() {
                        log::debug!("removing container path at {extra_container_path}");
                        std::fs::remove_dir_all(&extra_container_path)
                            .context(format!("removing {extra_container_path}"))?;
                    }
                }

                (desired_state, WorkloadStatePayload::None)
            }

            WorkloadState::Updated
            | WorkloadState::Reported
            | WorkloadState::Assigned
            | WorkloadState::Pending
            | WorkloadState::Updating
            | WorkloadState::Error(_)
            | WorkloadState::Unknown(_) => {
                anyhow::bail!("unsupported desired state {desired_state:?}")
            }
        };

        Ok((
            WorkloadStatus {
                id: workload._id,
                desired: workload.status.desired.clone(),
                actual: actual_status.clone(),
                payload: status_payload,
            },
            workload,
        ))
    }

    pub async fn update_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let try_message_payload =
            Self::convert_msg_to_type::<WorkloadResult>(msg).inspect(|message_payload| {
                log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload)
            });

        // TODO: throwing an actual error from here leads to the request silently skipped with no logs entry in the host-agent.
        let (workload_status, maybe_workload) =
            match Self::handle_workload_command(msg_subject, try_message_payload).await {
                Ok(result) => (result.0, Some(result.1)),
                Err(err) => {
                    let (status, maybe_workload) = match err.downcast::<WorkloadResultError>() {
                        Ok(WorkloadResultError { e, workload_result }) => (
                            WorkloadStatus {
                                actual: WorkloadState::Error(e.to_string()),
                                ..workload_result.status
                            },
                            workload_result.workload,
                        ),
                        Err(e) => (
                            WorkloadStatus {
                                id: None,
                                desired: WorkloadState::Unknown(Default::default()),
                                actual: WorkloadState::Error(e.to_string()),
                                payload: Default::default(),
                            },
                            None,
                        ),
                    };

                    log::error!("{status:?}");
                    (status, maybe_workload)
                }
            };

        Ok(WorkloadApiResult {
            maybe_response_tags: None,

            result: WorkloadResult {
                status: workload_status,
                workload: maybe_workload,
            },
        })
    }

    // For host agent ? or elsewhere ?
    // TODO: Talk through with Stefan
    pub async fn fetch_workload_status(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let workload_result = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        let status = workload_result.status;

        // we're interested in the actual status on the system for this workload
        let workload =
            match workload_result.workload {
                Some(workload) => workload,
                None => return Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            actual: WorkloadState::Error(
                                "we need to know the workload information to get a status for it"
                                    .to_owned(),
                            ),
                            ..status
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                }),
            };

        // TODO: look up the status for the given workload

        // Send updated status:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status,
                workload: Some(workload),
            },
            maybe_response_tags: None,
        })
    }
}

mod util {
    use anyhow::Context;
    use bson::oid::ObjectId;
    use db_utils::schemas::{WorkloadManifest, WorkloadManifestHolochainDhtV1};
    use futures::{AsyncBufReadExt, StreamExt};
    use std::{path::PathBuf, process::Stdio, str::FromStr};
    use tokio::process::Command;

    use crate::{
        host_api::{get_container_name, HOLOCHAIN_ADMIN_PORT_DEFAULT},
        types::WorkloadResult,
    };

    pub fn get_workload_id(wr: &WorkloadResult) -> anyhow::Result<ObjectId> {
        wr.workload.as_ref().and_then(|w| w._id).ok_or_else(|| {
            anyhow::anyhow!("need a workload with an id to process the workload request")
        })
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

        let exists = match mode {
            EnsureWorkloadPathMode::Create => {
                std::fs::create_dir_all(&workload_path).map(|()| true)?
            }
            EnsureWorkloadPathMode::Observe => std::fs::exists(&workload_path)?,
        };

        let path = workload_path.to_str().map(ToString::to_string).ok_or_else(|| anyhow::anyhow!("{workload_path:?} is not a valid string, and we need to use it in string representation"))?;

        Ok((path, exists))
    }

    // transform the workload into something that can be executed
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
                    // TODO(feat): support these
                    // holochain_version,
                    ..
                } = *inner;

                // this is used to store the key=value pairs for the attrset that is passed to `.override attrs`
                let mut override_attrs = vec![
                    format!(r#"containerName = "{}""#, get_container_name(&workload_id)?),
                    format!(r#"adminWebsocketPort = {}"#, HOLOCHAIN_ADMIN_PORT_DEFAULT),
                    // TODO: clarify if we want to autostart the container uncoditionally
                    format!(r#"autoStart = true"#),
                ];

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

                let override_attrs_stringified = override_attrs.join("; ") + ";";

                log::debug!(
                    "passing the these overrides to holochain: '{override_attrs_stringified}'"
                );

                let nix_build_args = [
                    "--extra-experimental-features",
                    "nix-command flakes",
                    "--impure",
                    "--expr",
                    &format!(
                        r#"(builtins.getFlake "github:holo-host/holo-host/host-agent-workloads-container").packages.${{builtins.currentSystem}}.extra-container-holochain.override {{ {override_attrs_stringified} }}"#
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
