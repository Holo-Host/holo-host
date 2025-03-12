/*
Service Name: WORKLOAD
Subject: "WORKLOAD.>"
Provisioning Account: ADMIN
Importing Account: HPOS
Users: orchestrator & host
*/

#[cfg(test)]
mod tests;

pub mod host_api;
pub mod orchestrator_api;
pub mod types;

use anyhow::Result;
use async_nats::jetstream::ErrorCode;
use async_nats::Message;
use async_trait::async_trait;
use core::option::Option::None;
use db_utils::schemas::{WorkloadState, WorkloadStatus};
use nats_utils::types::ServiceError;
use serde::Deserialize;
use std::future::Future;
use std::{fmt::Debug, sync::Arc};
use types::{WorkloadApiResult, WorkloadResult};

pub const WORKLOAD_SRV_NAME: &str = "WORKLOAD_SERVICE";
pub const WORKLOAD_SRV_SUBJ: &str = "WORKLOAD";
pub const WORKLOAD_SRV_VERSION: &str = "0.0.1";
pub const WORKLOAD_SRV_DESC: &str = "This service handles the flow of Workload requests between the Developer and the Orchestrator, and between the Orchestrator and Host.";

#[async_trait]
pub trait WorkloadServiceApi
where
    Self: std::fmt::Debug + Clone + 'static,
{
    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        let subject = msg.subject.clone().into_string();

        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!("Failed to deserialize payload: {}", e);
            log::error!(
                "Deserialization error for subject '{}': {}",
                subject,
                err_msg
            );
            ServiceError::request(err_msg, Some(ErrorCode::BAD_REQUEST))
        })
    }

    // Helper function to standardize the processing of incoming workload messages
    async fn process_request<T, Fut>(
        &self,
        msg: Arc<Message>,
        desired_state: WorkloadState,
        error_state: impl Fn(String) -> WorkloadState + Send + Sync,
        cb_fn: impl Fn(T) -> Fut + Send + Sync,
    ) -> Result<WorkloadApiResult, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Clone + Send + Sync + Debug + 'static,
        Fut: Future<Output = Result<WorkloadApiResult, ServiceError>> + Send,
    {
        // Deserialize payload into the expected type
        let payload: T = Self::convert_msg_to_type::<T>(msg.clone())?;
        let subject = msg.subject.clone().into_string();

        // Call callback handler
        match cb_fn(payload.clone()).await {
            Ok(r) => Ok(r),
            Err(e) => {
                let err_msg = format!(
                    "Failed to process workload request. Subject={}, Payload={:?}",
                    subject, payload
                );
                log::error!("{}: {}", err_msg, e);

                // Return response for stream with error state
                Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: None,
                            desired: desired_state,
                            actual: error_state(format!("{}: {}", err_msg, e)),
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            }
        }
    }
}
