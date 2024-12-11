use super::microservices::jetstream::JsStreamService;
use anyhow::{anyhow, Result};
use async_nats::jetstream::context::PublishAckFuture;
use async_nats::jetstream::{self, stream::Config};
use async_nats::Message;
use async_trait::async_trait;
use futures::StreamExt;
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

type EndpointHandler = Arc<dyn Fn(&Message) -> Result<Vec<u8>, anyhow::Error> + Send + Sync>;
type AsyncEndpointHandler = Arc<
    dyn Fn(&Message) -> Pin<Box<dyn Future<Output = Result<Vec<u8>, anyhow::Error>> + Send>>
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
    async fn monitor(&self) -> Result<(), Box<dyn Error>>;
    async fn close(&self) -> Result<(), Box<dyn Error>>;
    async fn add_stream(&self, opts: &AddStreamOptions) -> Result<(), Box<dyn Error>>;
    async fn publish(&self, opts: &PublishOptions) -> Result<(), Box<dyn Error>>;
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

// Impl the `Debug` trait for the `ConsumerExt.config` field *only*
impl std::fmt::Debug for DefaultClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultClient")
            .field("url", &self.url)
            .field("name", &self.name)
            .field("client", &self.client)
            .field("js", &self.js)
            .field("js_services", &self.js_services)
            .field("service_log_prefix", &self.service_log_prefix)
            .finish()
    }
}

pub struct DefaultClient {
    url: String,
    name: String,
    on_msg_published_event: Option<EventHandler>,
    on_msg_failed_event: Option<EventHandler>,
    client: async_nats::Client,
    js: jetstream::Context,
    js_services: Option<Vec<JsStreamService>>,
    service_log_prefix: String,
}

impl DefaultClient {
    pub async fn new(
        nats_url: &str, // eg: "nats://user:pw@127.0.0.1:4222"
        name: &str,
        inbox_prefix: &str,
        ping_interval: Option<Duration>,
        request_timeout: Option<Duration>, // Defaults to 5s
        credentials_path: Option<String>,
        opts: Vec<ClientOption>, // NB: These opts should not be required for client instantiation
    ) -> Result<Self, async_nats::Error> {
        let client = match credentials_path {
            Some(p) => {
                let path = std::path::Path::new(&p);
                async_nats::ConnectOptions::new()
                    .credentials_file(path)
                    .await?
                    // .require_tls(true)
                    .name(name)
                    .ping_interval(ping_interval.unwrap_or(Duration::from_secs(120)))
                    .request_timeout(Some(request_timeout.unwrap_or(Duration::from_secs(10))))
                    .custom_inbox_prefix(inbox_prefix)
                    .connect(nats_url)
                    .await?
            }
            None => {
                async_nats::ConnectOptions::new()
                    // .require_tls(true)
                    .name(name)
                    .ping_interval(ping_interval.unwrap_or(std::time::Duration::from_secs(120)))
                    .request_timeout(Some(
                        request_timeout.unwrap_or(std::time::Duration::from_secs(10)),
                    ))
                    .custom_inbox_prefix(inbox_prefix)
                    .connect(nats_url)
                    .await?
            }
        };

        let service_log_prefix = format!("NATS-CLIENT-LOG::{}::", name);

        let mut default_client = DefaultClient {
            url: String::new(),
            name: name.to_string(),
            on_msg_published_event: None,
            on_msg_failed_event: None,
            client: client.clone(),
            js: jetstream::new(client),
            js_services: None,
            service_log_prefix: service_log_prefix.clone(),
        };

        for opt in opts {
            opt(&mut default_client);
        }

        log::info!(
            "{}Connected to NATS server at {}",
            service_log_prefix,
            default_client.url
        );
        Ok(default_client)
    }

    pub async fn add_js_services(mut self, js_services: Vec<JsStreamService>) -> Self {
        let mut current_services = self.js_services.unwrap_or_default();
        current_services.extend(js_services);
        self.js_services = Some(current_services);
        self
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
                    EndpointType::Async(handler) => handler(&msg).await,
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

    async fn monitor(&self) -> Result<(), Box<dyn Error>> {
        if let async_nats::connection::State::Disconnected = self.client.connection_state() {
            Err(Box::new(ErrClientDisconnected))
        } else {
            Ok(())
        }
    }

    async fn close(&self) -> Result<(), Box<dyn Error>> {
        self.client.drain().await?;
        Ok(())
    }

    async fn add_stream(&self, opts: &AddStreamOptions) -> Result<(), Box<dyn Error>> {
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

    async fn publish(&self, opts: &PublishOptions) -> Result<(), Box<dyn Error>> {
        let result = self
            .js
            .publish(opts.subject.clone(), opts.data.clone().into())
            .await;

        let now = Instant::now();
        let duration = now.elapsed();
        if let Err(err) = result {
            if let Some(ref on_failed) = self.on_msg_failed_event {
                on_failed(&opts.subject, duration);
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

// Helper:
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

// Client Options:
pub fn with_js_services(js_services: Vec<JsStreamService>) -> ClientOption {
    Box::new(move |c: &mut DefaultClient| {
        c.js_services = Some(js_services.to_owned());
    })
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
