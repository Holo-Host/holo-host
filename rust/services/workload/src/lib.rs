/*
Service Name: WORKLOAD
Subject: "WORKLOAD.>"
Provisioning Account: WORKLOAD
Users: orchestrator & host
*/

pub mod orchestrator_api;
pub mod host_api;
pub mod types;

use anyhow::Result;
use core::option::Option::None;
use async_nats::jetstream::ErrorCode;
use async_trait::async_trait;
use std::{fmt::Debug, sync::Arc};
use async_nats::Message;
use std::future::Future;
use serde::Deserialize;
use util_libs::{
    nats_js_client::{ServiceError, AsyncEndpointHandler, JsServiceResponse},
    db::schemas::{WorkloadState, WorkloadStatus}
};

pub const WORKLOAD_SRV_NAME: &str = "WORKLOAD";
pub const WORKLOAD_SRV_SUBJ: &str = "WORKLOAD";
pub const WORKLOAD_SRV_VERSION: &str = "0.0.1";
pub const WORKLOAD_SRV_DESC: &str = "This service handles the flow of Workload requests between the Developer and the Orchestrator, and between the Orchestrator and Host.";


#[async_trait]
pub trait WorkloadServiceApi
where
    Self: std::fmt::Debug + Clone + 'static,
{
    fn call<F, Fut>(
        &self,
        handler: F,
    ) -> AsyncEndpointHandler<types::ApiResult>
    where
        F: Fn(Self, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<types::ApiResult, ServiceError>> + Send + 'static,
        Self: Send + Sync
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> JsServiceResponse<types::ApiResult> {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }

    fn convert_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!("Error: Failed to deserialize payload. Subject='{}' Err={}", msg.subject, e);
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{} Code={:?}", err_msg, ErrorCode::BAD_REQUEST))
        })
        
    }

    // Helper function to streamline the processing of incoming workload messages
    // NB: Currently used to process requests for MongoDB ops and the subsequent db change streams these db edits create (via the mongodb<>nats connector)
    async fn process_request<T, Fut>(
        &self,
        msg: Arc<Message>,
        desired_state: WorkloadState,
        cb_fn: impl Fn(T) -> Fut + Send + Sync,
        error_state: impl Fn(String) -> WorkloadState + Send + Sync,
    ) -> Result<types::ApiResult, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Clone + Send + Sync + Debug + 'static,
        Fut: Future<Output = Result<types::ApiResult, ServiceError>> + Send,
    {
        // 1. Deserialize payload into the expected type
        let payload: T = Self::convert_to_type::<T>(msg.clone())?;

        // 2. Call callback handler
        Ok(match cb_fn(payload.clone()).await {
            Ok(r) => r,
            Err(e) => {
                let err_msg = format!("Failed to process Workload Service Endpoint. Subject={} Payload={:?}, Error={:?}", msg.subject, payload, e);
                log::error!("{}", err_msg);
                let status = WorkloadStatus {
                    id: None,
                    desired: desired_state,
                    actual: error_state(err_msg),
                };

                // 3. return response for stream
                types::ApiResult(status, None)
            }
        })
    }
}
