use super::jetstream_client::JsClient;
use anyhow::Result;
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::ErrorCode;
use async_nats::{HeaderMap, Message};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

pub type EventListener = Arc<Box<dyn Fn(&mut JsClient) + Send + Sync>>;
pub type EventHandler = Arc<Pin<Box<dyn Fn(&str, &str, Duration) + Send + Sync>>>;
pub type JsServiceResponse<T> = Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send>>;
pub type EndpointHandler<T> = Arc<dyn Fn(&Message) -> Result<T, ServiceError> + Send + Sync>;
pub type AsyncEndpointHandler<T> = Arc<
    dyn Fn(Arc<Message>) -> Pin<Box<dyn Future<Output = Result<T, ServiceError>> + Send>>
        + Send
        + Sync,
>;
pub type ResponseSubjectsGenerator =
    Arc<dyn Fn(HashMap<String, String>) -> Vec<String> + Send + Sync>;

pub trait EndpointTraits:
    Serialize
    + for<'de> Deserialize<'de>
    + Send
    + Sync
    + Clone
    + Debug
    + CreateTag
    + CreateResponse
    + 'static
{
}

pub trait CreateTag: Send + Sync {
    fn get_tags(&self) -> HashMap<String, String>;
}

pub trait CreateResponse: Send + Sync {
    fn get_response(&self) -> bytes::Bytes;
}

#[async_trait]
pub trait ConsumerExtTrait: Send + Sync + Debug + 'static {
    fn get_consumer(&self) -> PullConsumer;
    fn get_endpoint(&self) -> Box<dyn Any + Send + Sync>;
    fn get_response(&self) -> Option<ResponseSubjectsGenerator>;
}

#[async_trait]
impl<T> ConsumerExtTrait for ConsumerExt<T>
where
    T: EndpointTraits,
{
    fn get_consumer(&self) -> PullConsumer {
        self.consumer.clone()
    }
    fn get_endpoint(&self) -> Box<dyn Any + Send + Sync> {
        Box::new(self.handler.clone())
    }
    fn get_response(&self) -> Option<ResponseSubjectsGenerator> {
        self.response_subject_fn.clone()
    }
}

#[derive(Clone, derive_more::Debug)]
pub struct ConsumerExt<T>
where
    T: EndpointTraits,
{
    pub consumer: PullConsumer,
    pub handler: EndpointType<T>,
    #[debug(skip)]
    pub response_subject_fn: Option<ResponseSubjectsGenerator>,
}

#[derive(Clone, derive_more::Debug)]
pub struct ConsumerBuilder<T>
where
    T: EndpointTraits,
{
    pub name: String,
    pub subject: String,
    pub handler: EndpointType<T>,
    #[debug(skip)]
    pub response_subject_fn: Option<ResponseSubjectsGenerator>,
}

#[derive(Clone)]
pub struct ServiceConsumerBuilder<S, R>
where
    S: Serialize + Clone + AsRef<str>,
    R: EndpointTraits,
{
    pub name: String,
    pub subject: S,
    pub subject_prefix: Option<String>,
    pub async_handler: AsyncEndpointHandler<R>,
    pub response_subject_fn: Option<ResponseSubjectsGenerator>,
}

impl<S, R> ServiceConsumerBuilder<S, R>
where
    S: Serialize + Clone + AsRef<str>,
    R: EndpointTraits,
{
    pub fn new(name: String, subject: S, async_handler: AsyncEndpointHandler<R>) -> Self {
        Self {
            name,
            subject,
            subject_prefix: None,
            async_handler,
            response_subject_fn: None,
        }
    }

    pub fn with_subject_prefix(mut self, prefix: String) -> Self {
        self.subject_prefix = Some(prefix);
        self
    }

    pub fn with_response_subject_fn(mut self, fn_gen: ResponseSubjectsGenerator) -> Self {
        self.response_subject_fn = Some(fn_gen);
        self
    }
}

impl<S, R> From<ServiceConsumerBuilder<S, R>> for ConsumerBuilder<R>
where
    S: Serialize + Clone + AsRef<str>,
    R: EndpointTraits,
{
    fn from(value: ServiceConsumerBuilder<S, R>) -> Self {
        let subject = if let Some(prefix) = value.subject_prefix {
            format!("{prefix}.{}", value.subject.as_ref())
        } else {
            value.subject.as_ref().to_string()
        };

        Self {
            name: value.name.to_string(),
            subject,
            handler: EndpointType::Async(value.async_handler),
            response_subject_fn: value.response_subject_fn,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct JsStreamServiceInfo<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub service_subject: &'a str,
}

#[derive(Clone, Debug)]
pub struct LogInfo {
    pub prefix: String,
    pub service_name: String,
    pub service_subject: String,
    pub endpoint_name: String,
    pub endpoint_subject: String,
}

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
impl<T> TryFrom<Box<dyn Any + Send + Sync>> for EndpointType<T>
where
    T: EndpointTraits,
{
    type Error = anyhow::Error;

    fn try_from(value: Box<dyn Any + Send + Sync>) -> Result<Self, Self::Error> {
        if let Ok(endpoint) = value.downcast::<EndpointType<T>>() {
            Ok(*endpoint)
        } else {
            Err(anyhow::anyhow!("Failed to downcast to EndpointType"))
        }
    }
}

#[derive(Clone, Debug)]
pub enum Credentials {
    Path(std::path::PathBuf), // String = pathbuf as string
    Password(String, String),
    Token(String),
}

#[derive(Deserialize, Default)]
pub struct JsClientBuilder {
    pub nats_url: String,
    pub name: String,
    pub inbox_prefix: String,
    #[serde(default, skip_deserializing)]
    pub credentials: Option<Vec<Credentials>>,
    #[serde(default)]
    pub ping_interval: Option<Duration>,
    #[serde(default)]
    pub request_timeout: Option<Duration>, // Defaults to 5s
    #[serde(skip_deserializing)]
    pub listeners: Vec<EventListener>,
}

#[derive(Clone, Deserialize, Default)]
pub struct JsServiceBuilder {
    pub name: String,
    pub description: String,
    pub version: String,
    pub service_subject: String,
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

#[derive(Error, Debug, Clone)]
pub enum ServiceError {
    #[error("Request error: {message}")]
    Request {
        message: String,
        code: Option<ErrorCode>,
    },

    #[error("Database error: {source}")]
    Database {
        source: mongodb::error::Error,
        collection: Option<String>,
        operation: Option<String>,
    },

    #[error("NATS error: {message}")]
    NATS {
        message: String,
        subject: Option<String>,
    },

    #[error("Internal error: {message}")]
    Internal {
        message: String,
        context: Option<String>,
    },
}

impl ServiceError {
    /// Creates a new Request error with optional error code
    pub fn request(message: impl Into<String>, code: Option<ErrorCode>) -> Self {
        Self::Request {
            message: message.into(),
            code,
        }
    }

    /// Creates a new Database error with context
    pub fn database(
        error: mongodb::error::Error,
        collection: Option<String>,
        operation: Option<String>,
    ) -> Self {
        Self::Database {
            source: error,
            collection,
            operation,
        }
    }

    /// Creates a new NATS error with optional subject
    pub fn nats(message: impl Into<String>, subject: Option<String>) -> Self {
        Self::NATS {
            message: message.into(),
            subject,
        }
    }

    /// Creates a new Internal error with optional context
    pub fn internal(message: impl Into<String>, context: Option<String>) -> Self {
        Self::Internal {
            message: message.into(),
            context,
        }
    }

    /// Returns true if this is a Request error
    pub fn is_request(&self) -> bool {
        matches!(self, Self::Request { .. })
    }

    /// Returns true if this is a Database error
    pub fn is_database(&self) -> bool {
        matches!(self, Self::Database { .. })
    }

    /// Returns true if this is a NATS error
    pub fn is_nats(&self) -> bool {
        matches!(self, Self::NATS { .. })
    }

    /// Returns true if this is an Internal error
    pub fn is_internal(&self) -> bool {
        matches!(self, Self::Internal { .. })
    }

    /// Gets the error message without the error type prefix
    pub fn message(&self) -> String {
        match self {
            Self::Request { message, .. } => message.clone(),
            Self::Database { source, .. } => source.to_string(),
            Self::NATS { message, .. } => message.clone(),
            Self::Internal { message, .. } => message.clone(),
        }
    }
}

// Manual implementation of From instead of using #[from]
impl From<mongodb::error::Error> for ServiceError {
    fn from(error: mongodb::error::Error) -> Self {
        Self::Database {
            source: error,
            collection: None,
            operation: None,
        }
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(error: serde_json::Error) -> Self {
        Self::request(error.to_string(), Some(ErrorCode::BAD_REQUEST))
    }
}

impl From<bson::ser::Error> for ServiceError {
    fn from(error: bson::ser::Error) -> Self {
        Self::internal(
            error.to_string(),
            Some("BSON serialization failed".to_string()),
        )
    }
}

impl From<bson::de::Error> for ServiceError {
    fn from(error: bson::de::Error) -> Self {
        Self::internal(
            error.to_string(),
            Some("BSON deserialization failed".to_string()),
        )
    }
}
