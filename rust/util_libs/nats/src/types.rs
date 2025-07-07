use super::jetstream_client::JsClient;
use anyhow::{Context, Result};
use async_nats::jetstream::{consumer::PullConsumer, ErrorCode};
use async_nats::{AuthError, HeaderMap, Message, ServerAddr};
use async_trait::async_trait;
use bytes::Bytes;
use educe::Educe;
use futures::StreamExt;
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
    + GetSubjectTags
    + GetResponse
    + GetHeaderMap
    + 'static
{
}

pub trait GetSubjectTags: Send + Sync {
    fn get_subject_tags(&self) -> HashMap<String, String>;
}

pub trait GetResponse: Send + Sync {
    fn get_response(&self) -> bytes::Bytes;
}

pub trait GetHeaderMap: Send + Sync {
    fn get_header_map(&self) -> Option<HeaderMap>;
}

#[async_trait]
pub trait ConsumerExtTrait: Send + Sync + Debug + 'static {
    fn get_consumer(&self) -> PullConsumer;
    fn get_endpoint(&self) -> Box<dyn Any + Send + Sync>;
    fn get_response_subject_fn(&self) -> Option<ResponseSubjectsGenerator>;
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
    fn get_response_subject_fn(&self) -> Option<ResponseSubjectsGenerator> {
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

impl AsRef<ServerAddr> for DeServerAddr {
    fn as_ref(&self) -> &ServerAddr {
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

impl From<ServerAddr> for DeServerAddr {
    fn from(value: ServerAddr) -> Self {
        Self(value)
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

    #[error("Authentication error: {source}")]
    Authentication {
        source: AuthError,
        subject: Option<String>,
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

    #[error("Workload error: {message}")]
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

    /// Creates a new Authentication error with subject
    pub fn auth(error: AuthError, subject: Option<String>) -> Self {
        Self::Authentication {
            source: error,
            subject,
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

    /// Returns true if this is a Authentication error
    pub fn is_auth(&self) -> bool {
        matches!(self, Self::Authentication { .. })
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
            Self::Authentication { source, .. } => source.to_string(),
            Self::NATS { message, .. } => message.clone(),
            Self::Internal { message, .. } => message.clone(),
            Self::Workload { message, .. } => message.clone(),
        }
    }
}

// Manual implementation of From instead of using #[from] for mongodb::error::Error
impl From<mongodb::error::Error> for ServiceError {
    fn from(error: mongodb::error::Error) -> Self {
        Self::Database {
            source: error,
            collection: None,
            operation: None,
        }
    }
}

// Manual implementation of From instead of using #[from] for AuthError
impl From<AuthError> for ServiceError {
    fn from(error: AuthError) -> Self {
        Self::Authentication {
            source: error,
            subject: None,
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
#[derive(Debug, Deserialize, Clone, clap::Args, Educe)]
#[educe(Default)]
pub struct NatsRemoteArgs {
    #[clap(long, env = "NATS_PASSWORD_FILE")]
    pub nats_password_file: Option<PathBuf>,

    #[clap(long, env = "NATS_PASSWORD")]
    pub nats_password: Option<String>,

    #[clap(long, env = "NATS_USER")]
    pub nats_user: Option<String>,

    #[clap(long, env = "NATS_URL")]
    #[educe(Default( expression = DeServerAddr(ServerAddr::from_str(NATS_URL_DEFAULT).expect("default nats url to parse"))))]
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
        let maybe_user = self
            .nats_user
            .clone()
            .or(self.nats_url.username().map(ToString::to_string));
        let maybe_password = self
            .nats_password
            .clone()
            .or(self.nats_url.password().map(ToString::to_string));

        let maybe = match (maybe_user, maybe_password, &self.nats_password_file) {
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
    pub dna_hash: String,
    #[clap(long)]
    pub coordinatior_identifier: String,
    #[clap(long)]
    pub zome_name: String,
    #[clap(long)]
    pub zome_fn_name: String,
    #[clap(long)]
    pub payload: String,
}

impl HcHttpGwRequest {
    pub const DEFAULT_BASE: &str = "http://127.0.0.1:8090";

    /// Returns the URL path for the request towards the hc-http-gw
    // an example curl command would be: curl -4v "http://dev-host:8090/{{HUMM_HIVE_DNA_HASH}}/{{WORKLOAD_ID}}/content/list_by_hive_link?payload=$payload"
    pub fn get_checked_url(
        &self,
        maybe_base: Option<Url>,
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

        let mut url = maybe_base.unwrap_or(Url::parse(Self::DEFAULT_BASE)?);

        url.set_path(&format!(
            "/{dna_hash}/{coordinator_identifier}/{zome_name}/{zome_fn_name}"
        ));

        url.set_query(Some(&format!("payload={payload}")));

        Ok(url)
    }

    pub fn nats_subject_suffix(installed_app_id: &str) -> String {
        format!("HC_HTTP_GW.{installed_app_id}",)
    }

    pub fn nats_destination_subject(&self) -> String {
        format!(
            "WORKLOAD.{}",
            Self::nats_subject_suffix(&self.coordinatior_identifier)
        )
    }

    pub fn nats_reply_subject(&self) -> String {
        format!(
            "WORKLOAD.{}.reply",
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HcHttpGwResponseMsg {
    pub response_subject: Option<String>,
    pub response: HcHttpGwResponse,
}

/// send a request and asynchronously wait for the reply
pub async fn hc_http_gw_nats_request(
    nats_client: Arc<JsClient>,
    request: HcHttpGwRequest,
    mut converted_headers: async_nats::HeaderMap,
) -> anyhow::Result<HcHttpGwResponse> {
    let destination_subject = request.nats_destination_subject();
    log::trace!(
        "Generated destination subject for Holochain Gateway request: destination_subject={destination_subject:?}"
    );

    let reply_subject = request.nats_reply_subject();
    log::trace!(
        "Generated reply subject for Holochain Gateway request: reply_subject={reply_subject:?}"
    );

    let data = serde_json::to_string(&request)?;

    converted_headers.append(
        async_nats::HeaderName::from_static(
            crate::jetstream_service::JsStreamService::HEADER_NAME_REPLY_OVERRIDE,
        ),
        async_nats::HeaderValue::from_str(&reply_subject)?,
    );

    let _ack = nats_client
        .js_context
        .publish_with_headers(destination_subject.clone(), converted_headers, data.into())
        .await?;
    log::info!("request published");

    let mut response = nats_client.client.subscribe(reply_subject.clone()).await?;

    let msg = response
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("got no response on subject {reply_subject}"))?;

    let response: HcHttpGwResponse = serde_json::from_slice(&msg.payload)?;

    Ok(response)
}

impl GetSubjectTags for HcHttpGwResponseMsg {
    fn get_subject_tags(&self) -> HashMap<String, String> {
        self.response_subject
            .clone()
            .map(|response_subject| {
                [(response_subject, String::default())]
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl GetResponse for HcHttpGwResponseMsg {
    fn get_response(&self) -> bytes::Bytes {
        match serde_json::to_vec(&self.response) {
            Ok(r) => r.into(),
            Err(e) => e.to_string().into(),
        }
    }
}

impl GetHeaderMap for HcHttpGwResponseMsg {
    fn get_header_map(&self) -> Option<async_nats::HeaderMap> {
        None
    }
}

impl EndpointTraits for HcHttpGwResponseMsg {}

/// helpers to sanitize NATS names
/// see https://docs.nats.io/running-a-nats-service/nats_admin/jetstream_admin/naming
pub mod sanitization {
    const NATS_NAME_MAX_LENGTH: usize = 31;
    const NATS_NAME_PROHIBITED_CHARS: [char; 7] = [' ', '/', '\\', '.', '>', '*', '\t'];

    pub fn sanity_check_nats_name(name: &str) -> Result<(), async_nats::Error> {
        if name.len() > NATS_NAME_MAX_LENGTH {
            return Err(async_nats::Error::from(format!(
                "'{name}' must not be equal to or longer than {NATS_NAME_MAX_LENGTH} characters"
            )));
        }
        for prohibited_char in NATS_NAME_PROHIBITED_CHARS {
            if name.contains(prohibited_char) {
                return Err(async_nats::Error::from(format!(
                    "'{name}' must not contain '{prohibited_char}'"
                )));
            }
        }

        Ok(())
    }

    pub fn sanitize_nats_name(name: &str) -> String {
        let mut final_name = name.to_string();

        for char in NATS_NAME_PROHIBITED_CHARS {
            final_name = final_name.replace(char, "_");
        }

        final_name = if final_name.len() > NATS_NAME_MAX_LENGTH {
            final_name.split_at(NATS_NAME_MAX_LENGTH).0.to_owned()
        } else {
            final_name
        };

        if name != final_name {
            log::debug!("sanitization changed from {name} -> {final_name}");
        }

        final_name
    }
}
