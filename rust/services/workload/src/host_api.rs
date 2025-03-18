/*
Endpoints & Managed Subjects:
    - `install_workload`: handles the "WORKLOAD.<host_pukey>.install." subject
    - `update_workload`: handles the "WORKLOAD.<host_pukey>.update_installed" subject
    - `uninstall_workload`: handles the "WORKLOAD.<host_pukey>.uninstall." subject
    - `fetch_workload_status`: handles the "WORKLOAD.<host_pukey>.send_status" subject
*/

use crate::types::WorkloadResult;

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use anyhow::Result;
use async_nats::Message;
use core::option::Option::None;
use db_utils::schemas::{Workload, WorkloadState, WorkloadStatus};
use nats_utils::types::ServiceError;
use std::{fmt::Debug, sync::Arc};
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

        // TODO: consider status.actual to inform assumptions towards the current state
        // TODO: run a seperate thread to send status updates while this is processed

        let desired_state = &workload.status.desired;
        let actual_status = match desired_state {
            WorkloadState::Installed | WorkloadState::Running => {
                let (workload_path_toplevel, _) =
                    ensure_workload_path(&workload_id, None, util::EnsureWorkloadPathMode::Create)?;
                let extra_container_path = realize_extra_container_path(
                    workload_id,
                    workload.deployable.clone(),
                    (&workload_path_toplevel).into(),
                )
                .await?;

                // TODO: move this to the workload processing function
                let start_or_restart_if_desired = if let WorkloadState::Running = desired_state {
                    " --start --restart-changed"
                } else {
                    ""
                };

                bash(&format!(
                    "extra-container create {extra_container_path}{start_or_restart_if_desired}",
                ))
                .await?;

                desired_state
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

                    // TODO: remove workoad directory
                }

                desired_state
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

        // TODO: send a reply message

        Ok((
            WorkloadStatus {
                id: workload._id,
                desired: workload.status.desired.clone(),
                actual: actual_status.clone(),
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

        let workload_status = Self::convert_msg_to_type::<WorkloadResult>(msg)?.status;

        // Send updated status:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status: workload_status,
                workload: None,
            },
            maybe_response_tags: None,
        })
    }
}

mod util {
    use anyhow::Context;
    use bson::oid::ObjectId;
    use db_utils::schemas::{WorkloadDeployable, WorkloadDeployableHolochainDhtV1};
    use futures::{AsyncBufReadExt, StreamExt};
    use std::{path::PathBuf, process::Stdio, str::FromStr};
    use tokio::process::Command;

    use crate::types::WorkloadResult;

    pub fn get_workload_id(wr: &WorkloadResult) -> anyhow::Result<ObjectId> {
        wr.workload.as_ref().and_then(|w| w._id).ok_or_else(|| {
            anyhow::anyhow!("need a workload with an id to process the workload request")
        })
    }

    pub(crate) async fn bash(cmd: &str) -> anyhow::Result<()> {
        let mut workload_cmd = tokio::process::Command::new("/usr/bin/env");
        workload_cmd.args(["bash", "-c", cmd]);

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
        deployable: WorkloadDeployable,
        workload_path: PathBuf,
    ) -> anyhow::Result<String> {
        log::debug!("transforming {deployable:?} at {workload_path:?}");

        match deployable {
            WorkloadDeployable::None => {
                anyhow::bail!("cannot install anything without a deployable");
            }
            WorkloadDeployable::ExtraContainerBuildCmd { nix_args } => {
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
                    WorkloadDeployable::ExtraContainerStorePath {
                        store_path: path.into(),
                    },
                    workload_path,
                ))
                .await
            }

            WorkloadDeployable::ExtraContainerStorePath { store_path } => {
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
                    WorkloadDeployable::ExtraContainerPath {
                        extra_container_path: container_closure_path,
                    },
                    workload_path,
                ))
                .await
            }

            WorkloadDeployable::ExtraContainerPath {
                extra_container_path,
            } => Ok(extra_container_path),

            WorkloadDeployable::HolochainDhtV1(inner) => {
                // TODO: construct build cmd
                let WorkloadDeployableHolochainDhtV1 {
                    // happ_binary_url,
                    ..
                    // network_seed,
                    // memproof,
                    // bootstrap_server_urls,
                    // sbd_server_urls,
                    // holochain_feature_flags,
                    // holochain_version,
                } = *inner;

                // TODO: implement downloading in the container
                // TODO: save this for later
                // let happ_binary_path_tmp = ham::Ham::download_happ(&happ_binary_url)
                //     .await
                //     .context(format!("downloading {happ_binary_url:?}"))?;
                // let happ_binary_path = workload_path.join("happ.bundle");

                // tokio::fs::rename(&happ_binary_path_tmp, &happ_binary_path)
                //     .await
                //     .context(format!(
                //         "renaming {happ_binary_path_tmp:?} to {happ_binary_path:?}"
                //     ))?;

                let nix_build_args= [
                    "--extra-experimental-features",
                    "nix-command flakes",
                    "--impure",
                    "--expr",
                    &[
                        r#"(builtins.getFlake "github:holo-host/holo-host").packages.${builtins.currentSystem}.extra-container-holochain.override {"#,
                        // TODO: as this is the hostname we're limited to 11 characters. make sure it's unique
                        &format!(r#"containerName = "{}";"#, &workload_id.to_hex()[0..10]),
                        // TODO: add the specific workload arguments
                        "}",
                        ].join("")
                    ]
                    .into_iter().map(ToString::to_string).collect();

                Box::pin(realize_extra_container_path(
                    workload_id,
                    WorkloadDeployable::ExtraContainerBuildCmd {
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
