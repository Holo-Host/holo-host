/*
Endpoints & Managed Subjects:
    - `install_workload`: handles the "WORKLOAD.<host_pukey>.install." subject
    - `update_workload`: handles the "WORKLOAD.<host_pukey>.update_installed" subject
    - `uninstall_workload`: handles the "WORKLOAD.<host_pukey>.uninstall." subject
    - `send_workload_status`: handles the "WORKLOAD.<host_pukey>.send_status" subject
*/

use crate::types::WorkloadResult;

use super::{types::WorkloadApiResult, WorkloadServiceApi};
use anyhow::Result;
use async_nats::Message;
use core::option::Option::None;
use std::{fmt::Debug, sync::Arc};
use util_libs::{
    db::schemas::{WorkloadState, WorkloadStatus},
    nats_js_client::ServiceError,
};

#[derive(Debug, Clone, Default)]
pub struct HostWorkloadApi {}

impl WorkloadServiceApi for HostWorkloadApi {}

impl HostWorkloadApi {
    pub async fn install_workload(
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
                desired: WorkloadState::Running,
                actual: WorkloadState::Unknown("..".to_string()),
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
                actual: WorkloadState::Unknown("..".to_string()),
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

    pub async fn uninstall_workload(
        &self,
        msg: Arc<Message>,
    ) -> Result<WorkloadApiResult, ServiceError> {
        let msg_subject = msg.subject.clone().into_string();
        log::trace!("Incoming message for '{}'", msg_subject);

        let message_payload = Self::convert_msg_to_type::<WorkloadResult>(msg)?;
        log::debug!("Message payload '{}' : {:?}", msg_subject, message_payload);

        let status = if let Some(workload) = message_payload.workload {
            // TODO: Talk through with Stefan
            // 1. Connect to interface for Nix and instruct systemd to UNinstall workload...
            // nix_uninstall_with(workload_id)

            // 2. Respond to endpoint request
            WorkloadStatus {
                id: workload._id,
                desired: WorkloadState::Uninstalled,
                actual: WorkloadState::Unknown("..".to_string()),
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
    pub async fn send_workload_status(
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
