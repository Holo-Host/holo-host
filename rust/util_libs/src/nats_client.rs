use anyhow::{anyhow, Result};
use async_nats::jetstream::context::PublishAckFuture;
use async_nats::jetstream::{self, stream::Config};
use async_nats::Message;
use async_trait::async_trait;
use futures::StreamExt;
use serde::Deserialize;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type ClientOption = Box<dyn Fn(&mut DefaultClient)>;
pub type EventListener = Box<dyn Fn(&mut DefaultClient)>;
pub type EventHandler = Pin<Box<dyn Fn(&str, Duration) + Send + Sync>>;

pub type EndpointHandler = Arc<dyn Fn(&Message) -> Result<Vec<u8>, anyhow::Error> + Send + Sync>;
pub type AsyncEndpointHandler = Arc<
    dyn Fn(Arc<Message>) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub enum EndpointType {
    Sync(EndpointHandler),
    Async(AsyncEndpointHandler),
}

#[derive(Debug)]
pub struct ErrClientDisconnected;

impl fmt::Display for ErrClientDisconnected {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not reach nats: connection closed")
    }
}

impl Error for ErrClientDisconnected {}

#[async_trait]
pub trait Client: Send + Sync {
    fn name(&self) -> &str;
    async fn monitor(&self) -> Result<(), async_nats::Error>;
    async fn close(&self) -> Result<(), async_nats::Error>;
    async fn get_stream(
        &self,
        get_stream: &str,
    ) -> Result<jetstream::stream::Stream, async_nats::Error>;
    async fn add_stream(&self, opts: &AddStreamOptions) -> Result<(), async_nats::Error>;
    async fn publish(&self, opts: &PublishOptions) -> Result<(), async_nats::Error>;
}

#[derive(Clone, Debug)]
pub struct AddStreamOptions {
    pub stream_name: String,
}

#[derive(Clone, Debug)]
pub struct PublishOptions {
    pub subject: String,
    pub msg_id: String,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for DefaultClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultClient")
            .field("url", &self.url)
            .field("name", &self.name)
            .field("client", &self.client)
            .field("js", &self.js)
            .field("service_log_prefix", &self.service_log_prefix)
            .finish()
    }
}

pub struct DefaultClient {
    url: String,
    name: String,
    on_msg_published_event: Option<EventHandler>,
    on_msg_failed_event: Option<EventHandler>,
    pub client: async_nats::Client, // inner_client
    js: jetstream::Context,
    service_log_prefix: String,
}

#[derive(Deserialize, Default)]
pub struct NewDefaultClientParams {
    pub nats_url: String,
    pub name: String,
    pub inbox_prefix: String,
    #[serde(skip_deserializing)]
    pub opts: Vec<ClientOption>, // NB: These opts should not be required for client instantiation
    #[serde(default)]
    pub credentials_path: Option<String>,
    #[serde(default)]
    pub ping_interval: Option<Duration>,
    #[serde(default)]
    pub request_timeout: Option<Duration>, // Defaults to 5s
}

impl DefaultClient {
    pub async fn new(p: NewDefaultClientParams) -> Result<Self, async_nats::Error> {
        let client = match p.credentials_path {
            Some(cp) => {
                let path = std::path::Path::new(&cp);
                async_nats::ConnectOptions::new()
                    .credentials_file(path)
                    .await?
                    // .require_tls(true)
                    .name(&p.name)
                    .ping_interval(p.ping_interval.unwrap_or(Duration::from_secs(120)))
                    .request_timeout(Some(p.request_timeout.unwrap_or(Duration::from_secs(10))))
                    .custom_inbox_prefix(&p.inbox_prefix)
                    .connect(&p.nats_url)
                    .await?
            }
            None => {
                async_nats::ConnectOptions::new()
                    // .require_tls(true)
                    .name(&p.name)
                    .ping_interval(
                        p.ping_interval
                            .unwrap_or(std::time::Duration::from_secs(120)),
                    )
                    .request_timeout(Some(
                        p.request_timeout
                            .unwrap_or(std::time::Duration::from_secs(10)),
                    ))
                    .custom_inbox_prefix(&p.inbox_prefix)
                    .connect(&p.nats_url)
                    .await?
            }
        };

        let service_log_prefix = format!("NATS-CLIENT-LOG::{}::", p.name);

        let mut default_client = DefaultClient {
            url: p.nats_url,
            name: p.name,
            on_msg_published_event: None,
            on_msg_failed_event: None,
            client: client.clone(),
            js: jetstream::new(client),
            service_log_prefix: service_log_prefix.clone(),
        };

        for opt in p.opts {
            opt(&mut default_client);
        }

        log::info!(
            "{}Connected to NATS server at {}",
            service_log_prefix,
            default_client.url
        );
        Ok(default_client)
    }

    pub async fn health_check_stream(&self, stream_name: &str) -> Result<(), async_nats::Error> {
        if let async_nats::connection::State::Disconnected = self.client.connection_state() {
            return Err(Box::new(ErrClientDisconnected));
        }
        let stream = &self.js.get_stream(stream_name).await?;
        let info = stream.cached_info();
        log::debug!(
            "{}JetStream (cached) info: stream:{}, info:{:?}",
            self.service_log_prefix,
            stream_name,
            info
        );
        Ok(())
    }

    pub async fn delete_stream(&self, stream_name: &str) -> Result<(), async_nats::Error> {
        self.js.delete_stream(stream_name).await?;
        log::debug!(
            "{}Deleted JS stream: {}",
            self.service_log_prefix,
            stream_name
        );
        Ok(())
    }

    pub async fn subscribe(
        &self,
        subject: &str,
        handler: EndpointType,
    ) -> Result<(), async_nats::Error> {
        let mut subscription = self.client.subscribe(subject.to_string()).await?;
        let js_context = self.js.clone();
        let service_log_prefix = self.service_log_prefix.clone();

        tokio::spawn(async move {
            while let Some(msg) = subscription.next().await {
                // todo!: persist handler for reliability cases
                log::info!("{}Received message: {:?}", service_log_prefix, msg);

                let result = match handler.to_owned() {
                    EndpointType::Sync(handler) => handler(&msg),
                    EndpointType::Async(handler) => handler(Arc::new(msg.clone())).await,
                };

                let response_bytes: bytes::Bytes = match result {
                    Ok(response) => response.into(),
                    Err(err) => err.to_string().into(),
                };

                // (NB: Only return a response if a reply address exists...
                // Otherwise, the underlying NATS system will receive a message it can't broker and will panic!)
                if let Some(reply) = &msg.reply {
                    if let Err(err) = js_context.publish(reply.to_owned(), response_bytes).await {
                        log::error!(
                            "{}Failed to send reply upon successful message consumption: subj='{}', err={:?}",
                            service_log_prefix,
                            reply,
                            err
                        );
                    };
                }
            }
        });
        Ok(())
    }

    pub async fn publish_with_retry(
        &self,
        opts: &PublishOptions,
        retries: usize,
    ) -> Result<PublishAckFuture, async_nats::Error> {
        let r = retry(
            || async {
                self.js
                    .publish(opts.subject.clone(), opts.data.clone().into())
                    .await
                    .map_err(|err| {
                        let e: anyhow::Error = err.into();
                        e
                    })
            },
            retries,
        )
        .await?;

        Ok(r)
    }
}

#[async_trait]
impl Client for DefaultClient {
    fn name(&self) -> &str {
        &self.name
    }

    async fn monitor(&self) -> Result<(), async_nats::Error> {
        if let async_nats::connection::State::Disconnected = self.client.connection_state() {
            Err(Box::new(ErrClientDisconnected))
        } else {
            Ok(())
        }
    }

    async fn close(&self) -> Result<(), async_nats::Error> {
        self.client.drain().await?;
        Ok(())
    }

    async fn add_stream(&self, opts: &AddStreamOptions) -> Result<(), async_nats::Error> {
        let config = Config {
            name: opts.stream_name.clone(),
            subjects: vec![format!("{}.*", opts.stream_name)],
            storage: jetstream::stream::StorageType::File,
            ..Default::default()
        };

        self.js.create_stream(config).await?;
        log::debug!(
            "{}Added JS stream: {}",
            self.service_log_prefix,
            opts.stream_name
        );
        Ok(())
    }

    async fn get_stream(
        &self,
        stream_name: &str,
    ) -> Result<jetstream::stream::Stream, async_nats::Error> {
        Ok(self.js.get_stream(stream_name).await?)
    }

    async fn publish(&self, opts: &PublishOptions) -> Result<(), async_nats::Error> {
        let result = self
            .js
            .publish(opts.subject.clone(), opts.data.clone().into())
            .await;

        let now = Instant::now();
        let duration = now.elapsed();
        if let Err(err) = result {
            if let Some(ref on_failed) = self.on_msg_failed_event {
                on_failed(&opts.subject, duration); // add msg_id
            }
            return Err(Box::new(err));
        }

        log::debug!(
            "{}Published message: subj={}, msg_id={} data={:?}",
            self.service_log_prefix,
            opts.subject,
            opts.msg_id,
            opts.data
        );
        if let Some(ref on_published) = self.on_msg_published_event {
            on_published(&opts.subject, duration);
        }
        Ok(())
    }
}

async fn retry<F, Fut, T>(mut operation: F, retries: usize) -> Result<T, anyhow::Error>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
    let mut attempts = 0;
    while attempts < retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(_) if attempts + 1 < retries => {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempts as u32))).await;
                attempts += 1;
            }
            Err(err) => return Err(err),
        }
    }
    Err(anyhow!("Exceeded max retries"))
}

pub fn with_event_listeners(listeners: Vec<EventListener>) -> ClientOption {
    Box::new(move |c: &mut DefaultClient| {
        for listener in &listeners {
            listener(c);
        }
    })
}

// Event Listener Options:
pub fn on_msg_published_event<F>(f: F) -> EventListener
where
    F: Fn(&str, Duration) + Send + Sync + Clone + 'static,
{
    Box::new(move |c: &mut DefaultClient| {
        c.on_msg_published_event = Some(Box::pin(f.clone()));
    })
}

pub fn on_msg_failed_event<F>(f: F) -> EventListener
where
    F: Fn(&str, Duration) + Send + Sync + Clone + 'static,
{
    Box::new(move |c: &mut DefaultClient| {
        c.on_msg_failed_event = Some(Box::pin(f.clone()));
    })
}

// Helpers:
pub fn get_nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "127.0.0.1:4111".to_string())
}

pub fn get_nats_client_creds(operator: &str, account: &str, user: &str) -> String {
    std::env::var("HOST_CREDS_FILE_PATH").unwrap_or_else(|_| {
        format!(
            "/.local/share/nats/nsc/keys/creds/{}/{}/{}.creds",
            operator, account, user
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    pub fn get_default_params() -> NewDefaultClientParams {
        NewDefaultClientParams {
            nats_url: "localhost:4222".to_string(),
            name: "test_client".to_string(),
            inbox_prefix: "_UNIQUE_INBOX".to_string(),
            credentials_path: None,
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
            opts: vec![],
        }
    }

    #[tokio::test]
    async fn test_nats_client_init() {
        let params = get_default_params();
        let client = DefaultClient::new(params).await;
        assert!(client.is_ok(), "Client initialization failed: {:?}", client);

        let client = client.unwrap();
        assert_eq!(client.name(), "test_client");
    }

    #[tokio::test]
    async fn test_nats_client_add_stream() {
        let params = get_default_params();
        let client = DefaultClient::new(params).await.unwrap();
        let add_stream_options = AddStreamOptions {
            stream_name: "test_stream".to_string(),
        };

        let result = client.add_stream(&add_stream_options).await;
        assert!(result.is_ok(), "Adding new stream failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_nats_client_publish() {
        let params = get_default_params();
        let client = DefaultClient::new(params).await.unwrap();
        let publish_options = PublishOptions {
            subject: "test_subject".to_string(),
            msg_id: "test_msg".to_string(),
            data: b"Hello, NATS!".to_vec(),
        };

        let result = client.publish(&publish_options).await;
        assert!(result.is_ok(), "Publishing message failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_nats_client_subscribe_and_receive_message() {
        let params = get_default_params();
        let client = DefaultClient::new(params).await.unwrap();

        // Set default message to false
        let message_received = Arc::new(tokio::sync::Mutex::new(false));

        async fn create_closure_async_block(
            message: Arc<tokio::sync::Mutex<bool>>,
        ) -> AsyncEndpointHandler {
            Arc::new(move |_msg: Arc<Message>| {
                let message = message.clone();
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    let mut flag = message.lock().await;
                    *flag = true;
                    Ok(vec![])
                })
            })
        }

        // Update message from false to true via handler to test the handler
        let handler =
            EndpointType::Async(create_closure_async_block(message_received.clone()).await);

        let subject = "test_subject";
        let subscription_result = client.subscribe(subject, handler).await;
        assert!(
            subscription_result.is_ok(),
            "Error: Subscription failed: {:?}",
            subscription_result
        );

        // Publish a message to the subject
        let publish_options = PublishOptions {
            subject: subject.to_string(),
            msg_id: "test_msg".to_string(),
            data: b"Test Message".to_vec(),
        };
        let publication_result = client.publish(&publish_options).await;
        assert!(
            publication_result.is_ok(),
            "Error: Publishing message failed: {:?}",
            publication_result
        );

        tokio::time::sleep(Duration::from_secs(1)).await;

        // Assert the message was received by the handler (ie: msg_flag should return true)
        let msg_flag = message_received.lock().await;
        assert!(*msg_flag);
    }

    #[tokio::test]
    async fn test_nats_client_publish_with_retry() {
        let params = get_default_params();
        let client = DefaultClient::new(params).await.unwrap();

        let publish_options = PublishOptions {
            subject: "test_subject".to_string(),
            msg_id: "retry_msg".to_string(),
            data: b"Retry Test".to_vec(),
        };
        let publication_result = client.publish_with_retry(&publish_options, 3).await;
        assert!(
            publication_result.is_ok(),
            "Publish with retry failed: {:?}",
            publication_result
        );
    }
}
