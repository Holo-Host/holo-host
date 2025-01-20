/*
Service Name: AUTH
Subject: "AUTH.>"
Provisioning Account: AUTH Account
Importing Account: Auth/NoAuth Account

This service should be run on the ORCHESTRATOR side and called from the HPOS side.
The NoAuth/Auth Server will import this service on the hub side and read local jwt files once the agent is validated.
NB: subject pattern = "<SERVICE_NAME>.<Subject>.<DirectObject>.<Verb>.<Details>"
This service handles the the "AUTH.<host_id>.file.transfer.JWT-<hoster_pubkey>.<chunk_id>" subject
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
use util_libs::nats_js_client::{ServiceError, AsyncEndpointHandler, JsServiceResponse};

pub const AUTH_SRV_NAME: &str = "AUTH";
pub const AUTH_SRV_SUBJ: &str = "AUTH";
pub const AUTH_SRV_VERSION: &str = "0.0.1";
pub const AUTH_SRV_DESC: &str =
    "This service handles the Authentication flow the HPOS and the Orchestrator.";

#[async_trait]
pub trait AuthServiceApi
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

}
