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
use core::option::Option::None;
use db_utils::schemas::{WorkloadDeployable, WorkloadState, WorkloadStatus};
use nats_utils::types::ServiceError;
use std::{fmt::Debug, sync::Arc};
use util::{bash, ensure_workload_path, get_workload_id, transform_workload_deployable};

#[derive(Debug, Clone, Default)]
pub struct HostWorkloadApi {}

impl WorkloadServiceApi for HostWorkloadApi {}

impl HostWorkloadApi {
    pub async fn install_workload(&self, msg: Arc<Message>) -> anyhow::Result<WorkloadApiResult> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let message_payload = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        // TODO(correctness): match on the actual status and only install if appropriate
        // match message_payload.status.actual {
        // }

        let workload_id = get_workload_id(&message_payload)?;

        let status = if let Some(workload) = message_payload.workload {
            let workload_path_toplevel =
                ensure_workload_path(&workload_id, None, util::EnsureWorkloadPathMode::CreateNew)?;
            let deployable =
                transform_workload_deployable(workload.deployable, workload_path_toplevel.into())
                    .await?;

            let result = match (deployable, &workload.status.desired) {
                (WorkloadDeployable::ExtraContainerPath { path }, WorkloadState::Installed) => {
                    let workload_path = ensure_workload_path(
                        &workload_id,
                        Some("extra-container"),
                        util::EnsureWorkloadPathMode::CreateNew,
                    )?;

                    std::os::unix::fs::symlink(&path, &workload_path).context(format!(
                        "linking workload_path from {path:?} to {workload_path:?}"
                    ))?;

                    bash(&format!(
                        "extra-container create {}",
                        path.to_string_lossy()
                    ))
                    .await
                }
                (WorkloadDeployable::ExtraContainerPath { path }, WorkloadState::Running) => {
                    bash(&format!(
                        "extra-container create {} --start --restart-changed",
                        path.to_string_lossy(),
                    ))
                    .await
                }

                other => Err(ServiceError::Workload {
                    message: format!("unsupported deployable/state combination: {other:?}"),
                    context: None,
                }),
            };

            WorkloadStatus {
                id: workload._id,
                desired: workload.status.desired.clone(),
                actual: match result {
                    Ok(_) => workload.status.desired,
                    Err(e) => WorkloadState::Error(e.to_string()),
                },
            }
        } else {
            let err_msg = format!("Failed to process Workload Service Endpoint. Subject={} Error=No workload found in message.", msg_subject);
            log::error!("{}", err_msg);
            WorkloadStatus {
                id: None,
                desired: WorkloadState::Updating,
                actual: WorkloadState::Error(err_msg),
            }
        };

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status,
                workload: None,
            },
            maybe_response_tags: None,
        })
    }

    pub async fn update_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let message_payload = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let status = if let Some(workload) = message_payload.workload {
            // TODO: Talk through with Stefan
            // 1. Connect to interface for Nix and instruct systemd to install workload...
            // eg: nix_install_with(workload)

            // TODO: associate the new workload state locally with workload._id

            // 2. Respond to endpoint request
            WorkloadStatus {
                id: workload._id,
                desired: WorkloadState::Updating,
                actual: WorkloadState::Error("unimplemented".to_string()),
            }
        } else {
            let err_msg = format!("Failed to process Workload Service Endpoint. Subject={} Error=No workload found in message.", msg_subject);
            log::error!("{}", err_msg);
            WorkloadStatus {
                id: None,
                desired: WorkloadState::Updating,
                actual: WorkloadState::Error(err_msg),
            }
        };

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status,
                workload: None,
            },
            maybe_response_tags: None,
        })
    }

    pub async fn uninstall_workload(&self, msg: Arc<Message>) -> anyhow::Result<WorkloadApiResult> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let message_payload = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let workload_id = get_workload_id(&message_payload)?;

        let status = if let Some(workload) = message_payload.workload {
            let result = match &workload.status.desired {
                WorkloadState::Deleted => {
                    let workload_path = ensure_workload_path(
                        &workload_id,
                        Some("extra-container"),
                        util::EnsureWorkloadPathMode::Exists,
                    )?;

                    bash(&format!("extra-container destroy {}", workload_path)).await
                }

                other => Err(ServiceError::Workload {
                    message: format!("unsupported desired state: {other:?}"),
                    context: None,
                }),
            };

            WorkloadStatus {
                id: workload._id,
                desired: workload.status.desired.clone(),
                actual: match result {
                    Ok(_) => workload.status.desired,
                    Err(e) => {
                        log::error!("error uninstalling workload: {e}");
                        WorkloadState::Error(e.to_string())
                    }
                },
            }
        } else {
            let err_msg = format!("Failed to process Workload Service Endpoint. Subject={} Error=No workload found in message.", msg_subject);
            log::error!("{}", err_msg);
            WorkloadStatus {
                id: None,
                desired: WorkloadState::Uninstalled,
                actual: WorkloadState::Error(err_msg),
            }
        };

        Ok(WorkloadApiResult {
            result: WorkloadResult {
                status,
                workload: None,
            },
            maybe_response_tags: None,
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
    use nats_utils::types::ServiceError;
    use std::{path::PathBuf, str::FromStr};
    use tokio::process::Command;

    use crate::types::WorkloadResult;

    pub fn get_workload_id(wr: &WorkloadResult) -> anyhow::Result<ObjectId> {
        wr.workload.as_ref().and_then(|w| w._id).ok_or_else(|| {
            anyhow::Error::from(ServiceError::Workload {
                message: "need a workload with an id to process the workload request".to_owned(),
                context: None,
            })
        })
    }

    pub(crate) async fn bash(cmd: &str) -> Result<(), ServiceError> {
        let mut workload_cmd = tokio::process::Command::new("/usr/bin/env");
        workload_cmd.args(["bash", "-c", cmd]);

        let output = workload_cmd
            .output()
            .await
            .map_err(|e| ServiceError::Workload {
                message: format!("error running {workload_cmd:?}: {e}"),
                context: None,
            })?;

        if !output.status.success() {
            return Err(ServiceError::Workload {
                message: format!(
                    "error running {workload_cmd:?} yielded non-success status:\n{output:?}",
                ),
                context: None,
            });
        }

        log::info!("workload creation result:\n{output:#?}");

        Ok(())
    }

    pub(crate) enum EnsureWorkloadPathMode {
        Create,
        CreateNew,
        Exists,
    }

    pub(crate) fn ensure_workload_path(
        id: &ObjectId,
        maybe_subdir: Option<&str>,
        mode: EnsureWorkloadPathMode,
    ) -> anyhow::Result<String> {
        const WORKLOAD_BASE_PATH: &str = "/var/lib/host-agent/workloads";

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

        match mode {
            EnsureWorkloadPathMode::Create => std::fs::create_dir_all(&workload_path)?,
            EnsureWorkloadPathMode::Exists => {
                if !std::fs::exists(&workload_path)
                    .context(format!("checking for the existence of {workload_path:?}"))?
                {
                    anyhow::bail!("{workload_path:?} doesn't exist");
                }
            }
            EnsureWorkloadPathMode::CreateNew => {
                if std::fs::exists(&workload_path)
                    .context(format!("checking for the existence of {workload_path:?}"))?
                {
                    anyhow::bail!("{workload_path:?} already exists");
                }

                std::fs::create_dir_all(&workload_path)?
            }
        }

        workload_path.to_str().map(ToString::to_string).ok_or_else(|| anyhow::anyhow!("{workload_path:?} is not a valid string, and we need to use it in string representation"))
    }

    // transform the workload into something that can be executed
    pub(crate) async fn transform_workload_deployable(
        deployable: WorkloadDeployable,
        workload_path: PathBuf,
    ) -> anyhow::Result<WorkloadDeployable> {
        match deployable {
            WorkloadDeployable::None => {
                anyhow::bail!("cannot install anything without a deployable");
            }
            WorkloadDeployable::ExtraContainerBuildCmd { nix_args } => {
                let output = {
                    let mut tokio_cmd = Command::new("/usr/bin/env");
                    tokio_cmd.arg("nix");
                    tokio_cmd.args(nix_args);

                    let msg = format!("running build command: {tokio_cmd:?}");
                    log::debug!("{}", msg);
                    tokio_cmd.output().await.context(msg)?
                };

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

                Box::pin(transform_workload_deployable(
                    WorkloadDeployable::ExtraContainerPath { path: path.into() },
                    workload_path,
                ))
                .await
            }
            WorkloadDeployable::HolochainDhtV1(inner) => {
                const INDEX: i32 = 0;
                // TODO: construct build cmd
                let WorkloadDeployableHolochainDhtV1 {
                    happ_binary_url,
                    ..
                    // network_seed,
                    // memproof,
                    // bootstrap_server_urls,
                    // sbd_server_urls,
                    // holochain_feature_flags,
                    // holochain_version,
                } = *inner;

                let happ_binary_path_tmp = ham::Ham::download_happ(&happ_binary_url)
                    .await
                    .context(format!("downloading {happ_binary_url:?}"))?;
                let happ_binary_path = workload_path.join("happ.bundle");

                tokio::fs::rename(&happ_binary_path_tmp, &happ_binary_path)
                    .await
                    .context(format!(
                        "renaming {happ_binary_path_tmp:?} to {happ_binary_path:?}"
                    ))?;

                let nix_args= [
                    "build",
                    "--no-link",
                    "--print-out-paths",
                    "--impure",
                    "--expr",
                    &[
                        r#"(builtins.getFlake "github:holo-host/holo-host").packages.\${builtins.currentSystem}.extra-container-holochain.override { "#,
                        // TODO: make index variable based on how many holochain containers the host already has locally
                        &format!("index = {INDEX};"),
                        // TODO: add workload arguments
                        "}",
                        ].join("")
                    ]
                    .into_iter().map(ToString::to_string).collect();

                Box::pin(transform_workload_deployable(
                    WorkloadDeployable::ExtraContainerBuildCmd { nix_args },
                    workload_path,
                ))
                .await
            }

            other => anyhow::bail!("unexpected case: {other:?}"),
        }
    }
}
