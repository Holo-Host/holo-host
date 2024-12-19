use anyhow::Result;
use async_nats::Message;
use authentication::AuthApi;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use util_libs::nats_js_client;

const USER_CREDENTIALS_PATH: &str = "./user_creds";

pub async fn save_user_jwt(auth_api: &AuthApi) -> nats_js_client::AsyncEndpointHandler {
    let api = auth_api.to_owned();
    // let user_name_clone = user_name.clone();
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                let api_clone = api.clone();
                Box::pin(async move {
                    api_clone.save_user_jwt(msg, USER_CREDENTIALS_PATH).await
                })
            },
        )
}

pub async fn save_hub_jwts(auth_api: &AuthApi) -> nats_js_client::AsyncEndpointHandler {
    let api = auth_api.to_owned();
    Arc::new(
            move |msg: Arc<Message>| -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>> {
                let api_clone = api.clone();
                Box::pin(async move {
                    api_clone.save_hub_jwts(msg).await
                })
            },
        )
}
