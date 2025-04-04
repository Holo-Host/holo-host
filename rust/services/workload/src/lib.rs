/*
Service Name: WORKLOAD
Subject: "WORKLOAD.>"
Provisioning Account: ADMIN
Importing Account: HPOS
Users: orchestrator & host

TODO(refactor) discuss the following alternative model:
    * [ ] design subjects and permissions so we can control forwarding per host
    * [ ] subject pattern:
        COMMAND.$OWNER.$SERVICE.$TASK
        EVENT.$OWNER.$FACT
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

// TODO: rename SRV -> SVC
pub const WORKLOAD_SRV_NAME: &str = "WORKLOAD_SERVICE";
pub const WORKLOAD_SRV_SUBJ: &str = "WORKLOAD";
pub const WORKLOAD_SRV_VERSION: &str = "0.0.1";
pub const WORKLOAD_SRV_DESC: &str = "This service handles the flow of Workload requests between the Developer and the Orchestrator, and between the Orchestrator and Host.";

// TODO(double-check): this was plural but i believe that's a bug because "assigned_host_0" does not start with "assigned_hosts"
pub const TAG_MAP_PREFIX_ASSIGNED_HOST: &str = "assigned_host";

pub const WORKLOAD_ORCHESTRATOR_SUBJECT_PREFIX: &str = "orchestrator";

#[async_trait]
pub trait WorkloadServiceApi
where
    Self: std::fmt::Debug + 'static,
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
                "Error deserializing into {} for subject '{}': {}",
                std::any::type_name::<T>(),
                subject,
                err_msg
            );
            ServiceError::request(err_msg, Some(ErrorCode::BAD_REQUEST))
        })
    }

    // Helper function to standardize the processing of incoming workload messages

    /* TODO(correctness):
        remove desired_ and error_state, and instead fill them in from result that is returned from  cb_fn.
        the assumptions are probably too strong. if we want to limit the values that can be inserted maybe we could find a way to handle that at insertion time. when we process the entry here it's already too late and the unexpected/invalid entry is persisted.

        the refactor can be done in a way where this function turns any Err(err) from the cb_fn into an Ok(WorkloadApiResult) that has sets the actual status to error the given details.
        by doing this, the cb_fn impls can use the rust native error handling instead of our custom one.
    */
    async fn process_request<T, Fut>(
        &self,
        msg: Arc<Message>,
        error_state: impl Fn(String) -> WorkloadState + Send + Sync,
        cb_fn: impl Fn(T) -> Fut + Send + Sync,
    ) -> Result<WorkloadApiResult, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Clone + Send + Sync + Debug + 'static,
        Fut: Future<Output = Result<WorkloadApiResult, ServiceError>> + Send,
    {
        // Deserialize payload into the expected type
        // TODO: we probably don't want to lose this error information. instead, for now, return it in the WorkloadStatus
        let payload: T = match Self::convert_msg_to_type::<T>(msg.clone()) {
            Ok(t) => t,
            Err(e) => {
                return Ok(WorkloadApiResult {
                    result: WorkloadResult {
                        status: WorkloadStatus {
                            id: None,
                            desired: WorkloadState::Unknown("cannot know this here".to_string()),
                            actual: error_state(format!("error converting message {msg:?}: {e}")),
                            payload: Default::default(),
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            }
        };
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
                            desired: WorkloadState::Unknown("cannot know this here".to_string()),
                            actual: error_state(format!("{}: {}", err_msg, e)),
                            payload: Default::default(),
                        },
                        workload: None,
                    },
                    maybe_response_tags: None,
                })
            }
        }
    }
}
