use super::jetstream_client::JsClient;
use anyhow::Result;
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::{AuthError, HeaderMap, Message};
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
    pub endpoint_subject: String,
    pub handler: EndpointType<T>,
    #[debug(skip)]
    pub response_subject_fn: Option<ResponseSubjectsGenerator>,
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

#[derive(Clone)]
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
    #[serde(default)]
    pub service_params: Vec<JsServiceBuilder>,
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
