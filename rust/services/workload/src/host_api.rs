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
use db_utils::schemas::{WorkloadState, WorkloadStatus};
use nats_utils::types::ServiceError;
use std::{fmt::Debug, sync::Arc};

#[derive(Debug, Clone, Default)]
pub struct HostWorkloadApi {}

impl WorkloadServiceApi for HostWorkloadApi {}

impl HostWorkloadApi {
    pub async fn install_workload(&self, msg: Arc<Message>) -> anyhow::Result<WorkloadApiResult> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let message_payload = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let status = if let Some(workload) = message_payload.workload {
            let result = match &workload.status.desired {
                WorkloadState::Installed => {
                    bash(&format!("extra-container create {}", workload.nix_pkg)).await
                }
                WorkloadState::Running => {
                    bash(&format!(
                        "extra-container create {} --start --restart-changed",
                        workload.nix_pkg
                    ))
                    .await
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

        let status = if let Some(workload) = message_payload.workload {
            let result = match &workload.status.desired {
                WorkloadState::Uninstalled => {
                    bash(&format!("extra-container destroy {}", workload.nix_pkg)).await
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

async fn bash(cmd: &str) -> Result<(), ServiceError> {
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
