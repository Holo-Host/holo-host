use anyhow::Result;
use async_nats::Message;
use authentication::AuthApi;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use util_libs::nats_js_client::AsyncEndpointHandler;

pub async fn add_user_pubkey(auth_api: &AuthApi) -> AsyncEndpointHandler {
    let api = auth_api.to_owned();
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                let api_clone = api.clone();
                Box::pin(async move {
                    api_clone.add_user_pubkey(msg).await
                })
            },
        )
}
