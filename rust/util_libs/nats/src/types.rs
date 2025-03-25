use super::jetstream_client::JsClient;
use anyhow::{Context, Result};
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::ErrorCode;
use async_nats::{HeaderMap, Message, ServerAddr};
use async_trait::async_trait;
use bytes::Bytes;
use educe::Educe;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use url::Url;

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
    T: EndpointTraits,
{
    Sync(EndpointHandler<T>),
    Async(AsyncEndpointHandler<T>),
}

impl<T> std::fmt::Debug for EndpointType<T>
where
    T: EndpointTraits,
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

pub const NATS_URL_DEFAULT: &str = "nats://127.0.0.1";

#[derive(Deserialize, Educe)]
#[educe(Default)]
pub struct JsClientBuilder {
    pub nats_remote_args: NatsRemoteArgs,
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

#[derive(Clone, Debug, Educe)]
#[educe(Deref)]
pub struct DeServerAddr(pub ServerAddr);
impl DeServerAddr {
    pub(crate) fn as_ref(&self) -> &ServerAddr {
        &self.0
    }
}

impl FromStr for DeServerAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(ServerAddr::from_str(s)?))
    }
}

impl From<&ServerAddr> for DeServerAddr {
    fn from(value: &ServerAddr) -> Self {
        Self(value.clone())
    }
}

impl<'a> Deserialize<'a> for DeServerAddr {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let url = Url::deserialize(deserializer)?;

        let server_addr = ServerAddr::from_url(url).map_err(serde::de::Error::custom)?;

        Ok(Self(server_addr))
    }
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

    #[error("Internal error: {message}")]
    Workload {
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
            Self::Workload { message, .. } => message.clone(),
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
#[derive(Deserialize, Clone, clap::Args, Educe)]
#[educe(Default)]
pub struct NatsRemoteArgs {
    #[clap(long, env = "NATS_PASSWORD_FILE")]
    pub nats_password_file: Option<PathBuf>,

    #[clap(long, env = "NATS_PASSWORD")]
    pub nats_password: Option<String>,

    #[clap(long, env = "NATS_USER")]
    pub nats_user: Option<String>,

    #[clap(long, env = "NATS_URL")]
    #[educe(Default( expression = DeServerAddr(ServerAddr::from_str(NATS_URL_DEFAULT).expect("default url parses"))))]
    pub nats_url: DeServerAddr,
    #[clap(
        long,
        default_value_t = false,
        env = "NATS_SKIP_TLS_VERIFICATION_DANGER"
    )]
    pub nats_skip_tls_verification_danger: bool,
}

impl NatsRemoteArgs {
    pub fn try_new(url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            nats_url: url.parse()?,

            ..Default::default()
        })
    }

    pub fn maybe_user_password(&self) -> anyhow::Result<Option<(String, String)>> {
        let maybe = match (
            &self.nats_user,
            &self.nats_password,
            &self.nats_password_file,
        ) {
            // incomplete data provided
            (None, None, None)
            | (None, None, Some(_))
            | (None, Some(_), None)
            | (Some(_), None, None)
            | (None, Some(_), Some(_)) => return Ok(None),

            // prefer password_file
            (Some(user), _, Some(password_file)) => {
                let pass = std::fs::read_to_string(password_file)
                    .context(format!("reading {password_file:?}"))?
                    .trim()
                    .to_string();

                log::debug!("user '{user}' and a password provided.");

                Some((user.clone(), pass))
            }
            (Some(user), Some(pass), None) => Some((user.clone(), pass.clone())),
        };

        Ok(maybe)
    }
}

/// This type is used for the request between the public facing HC HTTP API Gateway and the request handler running in the host-agent.
#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
pub struct HcHttpGwRequest {
    #[clap(long)]
    dna_hash: String,
    #[clap(long)]
    pub coordinatior_identifier: String,
    #[clap(long)]
    zome_name: String,
    #[clap(long)]
    zome_fn_name: String,
    #[clap(long)]
    payload: String,
}

impl HcHttpGwRequest {
    pub const DEFAULT_BASE: &str = "http://127.0.0.1:8090";

    /// Returns the URL path for the request towards the hc-http-gw
    pub fn get_checked_url(
        &self,
        maybe_base: Option<&str>,
        check_coordinator_identifier: &str,
    ) -> Result<Url> {
        let HcHttpGwRequest {
            dna_hash,
            payload,
            coordinatior_identifier: coordinator_identifier,
            zome_name,
            zome_fn_name,
        } = self;

        if coordinator_identifier != check_coordinator_identifier {
            anyhow::bail!("given coordinator identifier '{check_coordinator_identifier}' doesn't match '{coordinator_identifier}'");
        }

        let base = maybe_base.unwrap_or(Self::DEFAULT_BASE);

        // an example curl command would be: curl -4v "http://dev-host:8090/{{HUMM_HIVE_DNA_HASH}}/{{WORKLOAD_ID}}/content/list_by_hive_link?payload=$payload"
        let url_raw =format!("{base}/{dna_hash}/{coordinator_identifier}/{zome_name}/{zome_fn_name}?payload={payload}");

        let url = Url::parse(&url_raw).context(format!("parsing {url_raw} as Url"))?;

        Ok(url)
    }

    pub fn nats_subject_suffix(installed_app_id: &str) -> String {
        format!("HC_HTTP_GW.{installed_app_id}",)
    }

    pub fn nats_subject(&self) -> String {
        format!(
            // TODO: create a constant for this and figure out why it's not WORKLOAD
            "WORKLOAD.{}",
            Self::nats_subject_suffix(&self.coordinatior_identifier)
        )
    }
}

/// Response type for the HttpGwConsumer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HcHttpGwResponse {
    pub response_headers: HashMap<String, Box<[u8]>>,
    pub response_bytes: Bytes,
}

impl CreateTag for HcHttpGwResponse {
    fn get_tags(&self) -> HashMap<String, String> {
        // TODO
        HashMap::new()
    }
}

impl CreateResponse for HcHttpGwResponse {
    fn get_response(&self) -> bytes::Bytes {
        serde_json::to_vec(&self).unwrap().into()
    }
}

impl EndpointTraits for HcHttpGwResponse {}
