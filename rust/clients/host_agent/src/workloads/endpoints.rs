use anyhow::Result;
use async_nats::Message;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use util_libs::nats_js_client;
use workload::WorkloadApi;

pub async fn start_workload(workload_api: &WorkloadApi) -> nats_js_client::AsyncEndpointHandler {
    let api = workload_api.to_owned();
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                let api_clone = api.clone();
                Box::pin(async move {
                    api_clone.start_workload(msg).await
                })
            },
        )
}

pub async fn signal_status_update(
    workload_api: &WorkloadApi,
) -> nats_js_client::AsyncEndpointHandler {
    let api = workload_api.to_owned();
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                let api_clone = api.clone();
                Box::pin(async move {
                    api_clone.signal_status_update(msg).await
                })
            },
        )
}

pub async fn remove_workload(workload_api: &WorkloadApi) -> nats_js_client::AsyncEndpointHandler {
    let api = workload_api.to_owned();
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                let api_clone = api.clone();
                Box::pin(async move {
                    api_clone.remove_workload(msg).await
                })
            },
        )
}
