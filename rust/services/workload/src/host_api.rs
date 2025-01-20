/*
Endpoint Subjects:
- `start_workload`: handles the "WORKLOAD.<host_pukey>.start." subject
- `update_workload`: handles the "WORKLOAD.<host_pukey>.update_installed" subject
- `uninstall_workload`: handles the "WORKLOAD.<host_pukey>.uninstall." subject
- `send_workload_status`: handles the "WORKLOAD.<host_pukey>.send_status" subject
*/

use super::{types, WorkloadServiceApi};
use anyhow::Result;
use core::option::Option::None;
use std::{fmt::Debug, sync::Arc};
use async_nats::Message;
use util_libs::{
    nats_js_client::ServiceError,
    db::schemas::{self, WorkloadState, WorkloadStatus}
};

#[derive(Debug, Clone, Default)]
pub struct HostWorkloadApi {}

impl WorkloadServiceApi for HostWorkloadApi {}

impl HostWorkloadApi {
    pub async fn start_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.start' : {:?}", msg);
        let workload = Self::convert_to_type::<schemas::Workload>(msg)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to install workload...
        // eg: nix_install_with(workload)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            id: workload._id,
            desired: WorkloadState::Running,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(types::ApiResult(status, None))
    }

    pub async fn update_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.update_installed' : {:?}", msg);
        let workload = Self::convert_to_type::<schemas::Workload>(msg)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to install workload...
        // eg: nix_install_with(workload)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            id: workload._id,
            desired: WorkloadState::Updating,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(types::ApiResult(status, None))
    }

    pub async fn uninstall_workload(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!("Incoming message for 'WORKLOAD.uninstall' : {:?}", msg);
        let workload_id = Self::convert_to_type::<String>(msg)?;

        // TODO: Talk through with Stefan
        // 1. Connect to interface for Nix and instruct systemd to UNinstall workload...
        // nix_uninstall_with(workload_id)

        // 2. Respond to endpoint request
        let status = WorkloadStatus {
            id: Some(workload_id),
            desired: WorkloadState::Uninstalled,
            actual: WorkloadState::Unknown("..".to_string()),
        };
        Ok(types::ApiResult(status, None))
    }

    // For host agent ? or elsewhere ?
    // TODO: Talk through with Stefan
    pub async fn send_workload_status(&self, msg: Arc<Message>) -> Result<types::ApiResult, ServiceError> {
        log::debug!(
            "Incoming message for 'WORKLOAD.send_status' : {:?}",
            msg
        );

        let workload_status = Self::convert_to_type::<WorkloadStatus>(msg)?;

        // Send updated status:
        // NB: This will send the update to both the requester (if one exists)
        // and will broadcast the update to for any `response_subject` address registred for the endpoint
        Ok(types::ApiResult(workload_status, None))
    }
}
