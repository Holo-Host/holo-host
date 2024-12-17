use anyhow::Result;
use async_nats::Message;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use util_libs::nats_client;
use workload::api::WorkloadApi;

/// TODO:
pub async fn add_workload(workload_api: WorkloadApi) -> nats_client::AsyncEndpointHandler {
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                log::warn!("INCOMING Message for 'WORKLOAD.add' : {:?}", msg);
                let db_api = workload_api.clone();
                Box::pin(async move {
                    db_api.add_workload(msg).await
                })
            },
        )
}

/// TODO:
pub async fn handle_db_change() ->  nats_client::AsyncEndpointHandler {
    Arc::new(
        move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
            log::warn!("INCOMING Message for 'WORKLOAD.handle_change' : {:?}", msg);
            Box::pin(async move {
                // 1. Map over workload items in message and grab capacity requirements

                // 2. Call mongodb to get host/node info and filter by capacity availability

                // 3. Randomly choose host/node

                // 4. Respond to endpoint request
                let response = b"Successfully handled updated workload!".to_vec();
                Ok(response)
            })
        },
    )
}
