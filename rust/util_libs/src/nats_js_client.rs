use super::js_stream_service::{CreateTag, JsServiceParamsPartial, JsStreamService};
use crate::nats_server::LEAF_SERVER_DEFAULT_LISTEN_PORT;

use anyhow::Result;
use async_nats::{jetstream, AuthError, HeaderMap, Message, ServerInfo};
use core::marker::Sync;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(thiserror::Error, Debug, Clone)]
pub enum ServiceError {
    #[error("Request Error: {0}")]
    Request(String),
    #[error(transparent)]
    Database(#[from] mongodb::error::Error),
    #[error(transparent)]
    Authentication(#[from] AuthError),
    #[error("Nats Error: {0}")]
    NATS(String),
    #[error("Internal Error: {0}")]
    Internal(String),
}

pub type EventListener = Arc<Box<dyn Fn(&mut JsClient) + Send + Sync>>;
pub type EventHandler = Arc<Pin<Box<dyn Fn(&str, &str, Duration) + Send + Sync>>>;
pub type JsServiceResponse<T> = Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send>>;
pub type EndpointHandler<T> = Arc<dyn Fn(&Message) -> Result<T, ServiceError> + Send + Sync>;
pub type AsyncEndpointHandler<T> = Arc<
    dyn Fn(Arc<Message>) -> Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub enum EndpointType<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + CreateTag,
{
    Sync(EndpointHandler<T>),
    Async(AsyncEndpointHandler<T>),
}

impl<T> std::fmt::Debug for EndpointType<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone + Debug + CreateTag + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = match &self {
            EndpointType::Async(_) => "EndpointType::Async(<function>)",
            EndpointType::Sync(_) => "EndpointType::Sync(<function>)",
        };

        write!(f, "{}", t)
    }
}

// #[derive(Clone, Debug)]
// pub struct RequestInfo {
//     pub stream_subject: String,
//     pub consumer_name: String,
//     pub msg_id: String,
//     pub data: Vec<u8>,
//     pub headers: Option<HeaderMap>,
// }

#[derive(Clone, Debug)]
pub struct RequestInfo {
    pub stream_subject: String,
    pub consumer_name: String,
    pub msg_id: String,
    pub data: Vec<u8>,
    pub headers: Option<HeaderMap>,
}

#[derive(Clone, Debug)]
pub struct PublishInfo {
    pub subject: String,
    pub msg_id: String,
    pub data: Vec<u8>,
    pub headers: Option<HeaderMap>,
}

#[derive(Debug)]
pub struct ErrClientDisconnected;
impl fmt::Display for ErrClientDisconnected {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not reach nats: connection closed")
    }
}
impl Error for ErrClientDisconnected {}

impl std::fmt::Debug for JsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsClient")
            .field("url", &self.url)
            .field("name", &self.name)
            .field("client", &self.client)
            .field("js", &self.js)
            .field("js_services", &self.js_services)
            .field("service_log_prefix", &self.service_log_prefix)
            .finish()
    }
}

#[derive(Clone)]
pub struct JsClient {
    url: String,
    name: String,
    on_msg_published_event: Option<EventHandler>,
    on_msg_failed_event: Option<EventHandler>,
    client: async_nats::Client, // inner_client
    pub js: jetstream::Context,
    pub js_services: Option<Vec<JsStreamService>>,
    service_log_prefix: String,
}

#[derive(Clone)]
pub enum Credentials {
    Path(std::path::PathBuf), // String = pathbuf as string
    Password(String, String),
    Token(String),
}

#[derive(Deserialize, Default)]
pub struct NewJsClientParams {
    pub nats_url: String,
    pub name: String,
    pub inbox_prefix: String,
    pub service_params: Vec<JsServiceParamsPartial>,
    #[serde(skip_deserializing)]
    pub credentials: Option<Vec<Credentials>>,
    pub ping_interval: Option<Duration>,
    pub request_timeout: Option<Duration>, // Defaults to 5s
    #[serde(skip_deserializing)]
    pub listeners: Vec<EventListener>,
}

impl JsClient {
    pub async fn new(p: NewJsClientParams) -> Result<Self, async_nats::Error> {
        let mut connect_options: async_nats::ConnectOptions = async_nats::ConnectOptions::new()
            // .require_tls(true)
            .name(&p.name)
            .ping_interval(p.ping_interval.unwrap_or(Duration::from_secs(120)))
            .request_timeout(Some(p.request_timeout.unwrap_or(Duration::from_secs(10))))
            .custom_inbox_prefix(&p.inbox_prefix);

        if let Some(credentials_list) = p.credentials {
            for credentials in credentials_list {
                match credentials {
                    Credentials::Password(user, pw) => {
                        connect_options = connect_options.user_and_password(user, pw);
                    }
                    Credentials::Path(cp) => {
                        let path = std::path::Path::new(&cp);
                        connect_options = connect_options.credentials_file(path).await?;
                    }
                    Credentials::Token(t) => {
                        connect_options = connect_options.token(t);
                    }
                }
            }
        };

        let client = connect_options.connect(&p.nats_url).await?;

        let jetstream = jetstream::new(client.clone());
        let mut services = vec![];
        for params in p.service_params {
            let service = JsStreamService::new(
                jetstream.clone(),
                &params.name,
                &params.description,
                &params.version,
                &params.service_subject,
            )
            .await?;
            services.push(service);
        }

        let js_services = if services.is_empty() {
            None
        } else {
            Some(services)
        };

        let service_log_prefix = format!("NATS-CLIENT-LOG::{}::", p.name);

        let mut js_client = JsClient {
            url: p.nats_url,
            name: p.name,
            on_msg_published_event: None,
            on_msg_failed_event: None,
            client,
            js: jetstream,
            js_services,
            service_log_prefix: service_log_prefix.clone(),
        };

        for listener in p.listeners {
            listener(&mut js_client);
        }

        log::info!(
            "{}Connected to NATS server at {}",
            service_log_prefix,
            js_client.url
        );
        Ok(js_client)
    }

    pub fn get_server_info(&self) -> ServerInfo {
        self.client.server_info()
    }

    pub async fn get_stream_info(&self, stream_name: &str) -> Result<(), async_nats::Error> {
        let stream = &self.js.get_stream(stream_name).await?;
        let info = stream.get_info().await?;
        log::debug!(
            "{}JetStream info: stream:{}, info:{:?}",
            self.service_log_prefix,
            stream_name,
            info
        );
        Ok(())
    }

    pub async fn check_connection(
        &self,
    ) -> Result<async_nats::connection::State, async_nats::Error> {
        let conn_state = self.client.connection_state();
        if let async_nats::connection::State::Disconnected = conn_state {
            Err(Box::new(ErrClientDisconnected))
        } else {
            Ok(conn_state)
        }
    }

    pub async fn publish(
        &self,
        payload: PublishInfo,
    ) -> Result<(), async_nats::error::Error<async_nats::jetstream::context::PublishErrorKind>>
    {
        log::debug!(
            "{}Called Publish message: subj={}, msg_id={} data={:?}",
            self.service_log_prefix,
            payload.subject,
            payload.msg_id,
            payload.data
        );

        let now = Instant::now();
        let result = match payload.headers {
            Some(headers) => {
                self.js
                    .publish_with_headers(
                        payload.subject.clone(),
                        headers,
                        payload.data.clone().into(),
                    )
                    .await
            }
            None => {
                self.js
                    .publish(payload.subject.clone(), payload.data.clone().into())
                    .await
            }
        };

        let duration = now.elapsed();
        if let Err(err) = result {
            if let Some(ref on_failed) = self.on_msg_failed_event {
                on_failed(&payload.subject, &self.name, duration); // todo: add msg_id
            }
            return Err(err);
        }

        if let Some(ref on_published) = self.on_msg_published_event {
            on_published(&payload.subject, &self.name, duration);
        }
        Ok(())
    }

    pub async fn add_js_services(mut self, js_services: Vec<JsStreamService>) -> Self {
        let mut current_services = self.js_services.unwrap_or_default();
        current_services.extend(js_services);
        self.js_services = Some(current_services);
        self
    }

    pub async fn get_js_service(&self, js_service_name: String) -> Option<&JsStreamService> {
        if let Some(services) = &self.js_services {
            return services
                .iter()
                .find(|s| s.get_service_info().name == js_service_name);
        }
        None
    }

    pub async fn close(&self) -> Result<(), async_nats::Error> {
        self.client.drain().await?;
        Ok(())
    }
}

// Client Options:
pub fn with_event_listeners(listeners: Vec<EventListener>) -> EventListener {
    Arc::new(Box::new(move |c: &mut JsClient| {
        for listener in &listeners {
            listener(c);
        }
    }))
}

// Event Listener Options:
pub fn on_msg_published_event<F>(f: F) -> EventListener
where
    F: Fn(&str, &str, Duration) + Send + Sync + Clone + 'static,
{
    Arc::new(Box::new(move |c: &mut JsClient| {
        c.on_msg_published_event = Some(Arc::new(Box::pin(f.clone())));
    }))
}

pub fn on_msg_failed_event<F>(f: F) -> EventListener
where
    F: Fn(&str, &str, Duration) + Send + Sync + Clone + 'static,
{
    Arc::new(Box::new(move |c: &mut JsClient| {
        c.on_msg_failed_event = Some(Arc::new(Box::pin(f.clone())));
    }))
}

// Helpers:
// TODO: there's overlap with the NATS_LISTEN_PORT. refactor this to e.g. read NATS_LISTEN_HOST and NATS_LISTEN_PORT
pub fn get_nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| {
        let default = format!("127.0.0.1:{}", LEAF_SERVER_DEFAULT_LISTEN_PORT); // Shouldn't this be the 'NATS_LISTEN_PORT'?
        log::debug!("using default for NATS_URL: {default}");
        default
    })
}

fn get_nsc_root_path() -> String {
    std::env::var("NSC_PATH").unwrap_or_else(|_| "/.local/share/nats/nsc".to_string())
}

pub fn get_local_creds_path() -> String {
    std::env::var("LOCAL_CREDS_PATH")
        .unwrap_or_else(|_| format!("{}/local_creds", get_nsc_root_path()))
}

pub fn get_nats_creds_by_nsc(operator: &str, account: &str, user: &str) -> String {
    format!(
        "{}/keys/creds/{}/{}/{}.creds",
        get_nsc_root_path(),
        operator,
        account,
        user
    )
}

pub fn get_nats_jwt_by_nsc(operator: &str, account: &str, user: &str) -> String {
    format!(
        "{}/stores/{}/accounts/{}/users/{}.jwt",
        get_nsc_root_path(),
        operator,
        account,
        user
    )
}

pub fn get_event_listeners() -> Vec<EventListener> {
    // TODO: Use duration in handlers..
    let published_msg_handler = move |msg: &str, client_name: &str, _duration: Duration| {
        log::info!(
            "Successfully published message for {}. Msg: {:?}",
            client_name,
            msg
        );
    };
    let failure_handler = |err: &str, client_name: &str, _duration: Duration| {
        log::info!("Failed to publish for {}. Err: {:?}", client_name, err);
    };

    let event_listeners = vec![
        on_msg_published_event(published_msg_handler),
        on_msg_failed_event(failure_handler),
    ];

    event_listeners
}

#[cfg(feature = "tests_integration_nats")]
#[cfg(test)]
mod tests {
    use super::*;

    pub fn get_default_params() -> NewJsClientParams {
        NewJsClientParams {
            nats_url: "localhost:4222".to_string(),
            name: "test_client".to_string(),
            inbox_prefix: "_UNIQUE_INBOX".to_string(),
            service_params: vec![],
            credentials_path: None,
            ping_interval: Some(Duration::from_secs(10)),
            request_timeout: Some(Duration::from_secs(5)),
            opts: vec![],
        }
    }

    #[tokio::test]
    async fn test_nats_js_client_init() {
        let params = get_default_params();
        let client = JsClient::new(params).await;
        assert!(client.is_ok(), "Client initialization failed: {:?}", client);

        let client = client.unwrap();
        assert_eq!(client.name(), "test_client");
    }

    #[tokio::test]
    async fn test_nats_js_client_publish() {
        let params = get_default_params();
        let client = JsClient::new(params).await.unwrap();
        let payload = PublishInfo {
            subject: "test_subject".to_string(),
            msg_id: "test_msg".to_string(),
            data: b"Hello, NATS!".to_vec(),
        };

        let result = client.publish(&publish_options).await;
        assert!(result.is_ok(), "Publishing message failed: {:?}", result);
    }
}
