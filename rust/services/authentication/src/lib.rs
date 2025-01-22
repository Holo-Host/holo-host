/*
Service Name: AUTH
Subject: "AUTH.>"
Provisioning Account: AUTH Account (ie: This service is exclusively permissioned to the AUTH account.)
Users: orchestrator & noauth

*/

pub mod orchestrator_api;
pub mod host_api;
pub mod types;
pub mod utils;

use anyhow::Result;
use async_nats::Message;
use async_nats::jetstream::ErrorCode;
use async_trait::async_trait;
use std::sync::Arc;
use std::future::Future;
use serde::Deserialize;
use types::AuthApiResult;
use util_libs::nats_js_client::{ServiceError, AsyncEndpointHandler, JsServiceResponse};

pub const AUTH_SRV_NAME: &str = "AUTH";
pub const AUTH_SRV_SUBJ: &str = "AUTH";
pub const AUTH_SRV_VERSION: &str = "0.0.1";
pub const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the Host and the Orchestrator.";

#[async_trait]
pub trait AuthServiceApi
where
    Self: std::fmt::Debug + Clone + 'static,
{
    fn call<F, Fut>(
        &self,
        handler: F,
    ) -> AsyncEndpointHandler<AuthApiResult>
    where
        F: Fn(Self, Arc<Message>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AuthApiResult, ServiceError>> + Send + 'static,
        Self: Send + Sync
    {
        let api = self.to_owned(); 
        Arc::new(move |msg: Arc<Message>| -> JsServiceResponse<AuthApiResult> {
            let api_clone = api.clone();
            Box::pin(handler(api_clone, msg))
        })
    }

    fn convert_to_type<T>(data: Vec<u8>, msg_subject: &str) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        serde_json::from_slice::<T>(&data).map_err(|e| {
            let err_msg = format!("Error: Failed to deserialize payload data. Subject='{}' Err={}", msg_subject, e);
            log::error!("{}", err_msg);
            ServiceError::Internal(err_msg.to_string())
        })
        
    }

    fn convert_msg_to_type<T>(msg: Arc<Message>) -> Result<T, ServiceError>
    where
        T: for<'de> Deserialize<'de> + Send + Sync,
    {
        let payload_buf = msg.payload.to_vec();
        serde_json::from_slice::<T>(&payload_buf).map_err(|e| {
            let err_msg = format!("Error: Failed to deserialize payload. Subject='{}' Err={}", msg.subject.clone().into_string(), e);
            log::error!("{}", err_msg);
            ServiceError::Request(format!("{} Code={:?}", err_msg, ErrorCode::BAD_REQUEST))
        })
    }

}
