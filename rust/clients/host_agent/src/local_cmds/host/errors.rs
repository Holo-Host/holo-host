use crate::local_cmds::support::errors::SupportError;
use crate::remote_cmds::errors::RemoteError;
use async_nats::client::DrainError;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Main application error type that handles all possible errors
pub type HostAgentResult<T> = Result<T, HostAgentError>;

#[derive(Debug, thiserror::Error)]
pub enum HostAgentError {
    // Configuration and validation errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Configuration validation failed: {reason}")]
    Validation { reason: String },

    // Authentication errors with detailed context
    #[error("Authentication failed for device {device_id}: {reason}")]
    Authentication { device_id: String, reason: String },

    #[error("Credential validation failed: {details}")]
    CredentialValidation { details: String },

    #[error("Network connection failed: {url}")]
    NetworkError {
        url: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    // Service operation errors with context
    #[error("Service operation failed: {operation} - {reason}")]
    Service { operation: String, reason: String },

    #[error("Service unavailable: {service} - {reason}")]
    ServiceUnavailable { service: String, reason: String },

    // System and hardware errors
    #[error("System information unavailable: {component}")]
    SystemInfo { component: String },

    #[error("System operation failed: {operation}")]
    System {
        operation: String,
        #[source]
        source: anyhow::Error,
    },

    // File and I/O errors with context
    #[error("File operation failed: {operation} on {path}: {reason}")]
    FileOperation {
        operation: String,
        path: PathBuf,
        reason: String,
    },

    #[error("I/O operation failed: {operation}")]
    Io {
        operation: String,
        #[source]
        source: std::io::Error,
    },

    // NATS-specific errors with context
    #[error("NATS connection failed: {endpoint}")]
    NatsConnection {
        endpoint: String,
        #[source]
        source: async_nats::Error,
    },

    #[error("NATS request failed: {operation}")]
    NatsRequest {
        operation: String,
        #[source]
        source: async_nats::RequestError,
    },

    #[error("NATS publish failed: {subject}")]
    NatsPublish {
        subject: String,
        #[source]
        source: async_nats::PublishError,
    },

    // Cryptographic and security errors
    #[error("Cryptographic operation failed: {operation}")]
    Crypto {
        operation: String,
        #[source]
        source: nkeys::error::Error,
    },

    #[error("Signature verification failed: {details}")]
    SignatureVerification { details: String },

    // Timeout and timing errors
    #[error("Operation timed out: {operation} after {duration:?}")]
    Timeout {
        operation: String,
        duration: Duration,
    },

    // Serialization and parsing errors
    #[error("Serialization failed: {operation}")]
    Serialization {
        operation: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("JSON parsing failed: {context}")]
    JsonParsing {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    // Support and remote operation errors
    #[error("Support operation failed: {0}")]
    Support(#[from] SupportError),

    #[error("Remote operation failed: {0}")]
    Remote(#[from] RemoteError),

    // Workload-specific errors
    #[error("Workload operation failed: {operation}")]
    Workload {
        operation: String,
        #[source]
        source: workload::types::WorkloadError,
    },

    // Generic error for unexpected cases
    #[error("Unexpected error: {message}")]
    Unexpected { message: String },
}

impl HostAgentError {
    // Configuration errors
    pub fn config(message: &str) -> Self {
        Self::Config {
            message: message.to_string(),
        }
    }

    // Authentication errors with context
    pub fn auth_failed(device_id: &str, reason: &str) -> Self {
        Self::Authentication {
            device_id: device_id.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn auth_failed_simple(reason: &str) -> Self {
        Self::Authentication {
            device_id: "unknown".to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn credential_validation_failed(details: &str) -> Self {
        Self::CredentialValidation {
            details: details.to_string(),
        }
    }

    // Network errors with context
    pub fn network_error(url: &str, source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::NetworkError {
            url: url.to_string(),
            source,
        }
    }

    // Service errors with context
    pub fn service_failed(operation: &str, reason: &str) -> Self {
        Self::Service {
            operation: operation.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn service_unavailable(service: &str, reason: &str) -> Self {
        Self::ServiceUnavailable {
            service: service.to_string(),
            reason: reason.to_string(),
        }
    }

    // System errors with context
    pub fn system_info_unavailable(component: &str) -> Self {
        Self::SystemInfo {
            component: component.to_string(),
        }
    }

    pub fn system_operation_failed(operation: &str, source: anyhow::Error) -> Self {
        Self::System {
            operation: operation.to_string(),
            source,
        }
    }

    // File operation errors with context
    pub fn file_operation_failed(operation: &str, path: &Path, reason: &str) -> Self {
        Self::FileOperation {
            operation: operation.to_string(),
            path: path.to_path_buf(),
            reason: reason.to_string(),
        }
    }

    // NATS errors with context
    pub fn nats_connection_failed(endpoint: &str, source: async_nats::Error) -> Self {
        Self::NatsConnection {
            endpoint: endpoint.to_string(),
            source,
        }
    }

    pub fn nats_request_failed(operation: &str, source: async_nats::RequestError) -> Self {
        Self::NatsRequest {
            operation: operation.to_string(),
            source,
        }
    }

    pub fn nats_publish_failed(subject: &str, source: async_nats::PublishError) -> Self {
        Self::NatsPublish {
            subject: subject.to_string(),
            source,
        }
    }

    // Cryptographic errors with context
    pub fn crypto_operation_failed(operation: &str, source: nkeys::error::Error) -> Self {
        Self::Crypto {
            operation: operation.to_string(),
            source,
        }
    }

    pub fn signature_verification_failed(details: &str) -> Self {
        Self::SignatureVerification {
            details: details.to_string(),
        }
    }

    // Timeout errors with context
    pub fn timeout(operation: &str, duration: Duration) -> Self {
        Self::Timeout {
            operation: operation.to_string(),
            duration,
        }
    }

    // Serialization errors with context
    pub fn serialization_failed(operation: &str, source: serde_json::Error) -> Self {
        Self::Serialization {
            operation: operation.to_string(),
            source,
        }
    }

    pub fn json_parsing_failed(context: &str, source: serde_json::Error) -> Self {
        Self::JsonParsing {
            context: context.to_string(),
            source,
        }
    }

    // Workload errors with context
    pub fn workload_operation_failed(
        operation: &str,
        source: workload::types::WorkloadError,
    ) -> Self {
        Self::Workload {
            operation: operation.to_string(),
            source,
        }
    }

    // Validation errors
    pub fn validation(reason: &str) -> Self {
        Self::Validation {
            reason: reason.to_string(),
        }
    }

    // Unexpected errors
    pub fn unexpected(message: &str) -> Self {
        Self::Unexpected {
            message: message.to_string(),
        }
    }

    // Helper methods for common error patterns
    pub fn with_context(self, context: &str) -> Self {
        match self {
            Self::Service { operation, reason } => Self::Service {
                operation: format!("{}: {}", context, operation),
                reason,
            },
            Self::Validation { reason } => Self::Validation {
                reason: format!("{}: {}", context, reason),
            },
            Self::Authentication { device_id, reason } => Self::Authentication {
                device_id,
                reason: format!("{}: {}", context, reason),
            },
            _ => self,
        }
    }

    // Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NetworkError { .. }
                | Self::NatsConnection { .. }
                | Self::ServiceUnavailable { .. }
                | Self::Timeout { .. }
                | Self::Io { .. }
        )
    }
}

// Conversion to authentication crate's AuthError for signature operations
impl From<HostAgentError> for authentication::types::AuthError {
    fn from(err: HostAgentError) -> Self {
        match err {
            HostAgentError::Authentication { reason, .. } => {
                authentication::types::AuthError::auth_failed(&reason)
            }
            HostAgentError::Config { message } => {
                authentication::types::AuthError::config_error(&message)
            }
            HostAgentError::Crypto { source, .. } => {
                authentication::types::AuthError::signature_failed(&source.to_string())
            }
            _ => authentication::types::AuthError::signature_failed(&err.to_string()),
        }
    }
}

// Conversion to workload crate's WorkloadError for workload operations
impl From<HostAgentError> for workload::types::WorkloadError {
    fn from(err: HostAgentError) -> Self {
        match err {
            HostAgentError::Config { message } => {
                workload::types::WorkloadError::config_error(&message)
            }
            HostAgentError::ServiceUnavailable { reason, .. } => {
                workload::types::WorkloadError::service_error(&reason)
            }
            HostAgentError::Remote(RemoteError::Connection { reason, .. }) => {
                workload::types::WorkloadError::nats_failed(&reason)
            }
            HostAgentError::Remote(RemoteError::Operation { reason, .. }) => {
                workload::types::WorkloadError::workload_failed(&reason)
            }
            HostAgentError::Workload { source, .. } => source,
            _ => workload::types::WorkloadError::service_error(&err.to_string()),
        }
    }
}

// From implementations - replaces manual map_err usage across the crate
// Standard lib errors
impl From<std::io::Error> for HostAgentError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            operation: "I/O operation".to_string(),
            source: err,
        }
    }
}

impl From<std::env::VarError> for HostAgentError {
    fn from(err: std::env::VarError) -> Self {
        Self::Service {
            operation: "environment variable".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<std::string::FromUtf8Error> for HostAgentError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Service {
            operation: "UTF-8 conversion".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<Box<dyn std::error::Error>> for HostAgentError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self::Service {
            operation: "boxed error".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<std::convert::Infallible> for HostAgentError {
    fn from(_: std::convert::Infallible) -> Self {
        // This should never happen since Infallible can never be constructed
        unreachable!("Infallible error should never occur")
    }
}

// NATS errors
impl From<async_nats::SubscribeError> for HostAgentError {
    fn from(err: async_nats::SubscribeError) -> Self {
        Self::Remote(RemoteError::Connection {
            endpoint: "NATS subscription".to_string(),
            reason: err.to_string(),
        })
    }
}

impl From<async_nats::RequestError> for HostAgentError {
    fn from(err: async_nats::RequestError) -> Self {
        match err.kind() {
            async_nats::RequestErrorKind::TimedOut => {
                Self::timeout("authentication request", Duration::from_secs(30))
            }
            _ => Self::auth_failed_simple(&format!("Authentication request failed: {}", err)),
        }
    }
}

impl From<async_nats::PublishError> for HostAgentError {
    fn from(err: async_nats::PublishError) -> Self {
        Self::auth_failed_simple(&format!("Failed to publish message: {}", err))
    }
}

impl From<async_nats::header::ParseHeaderValueError> for HostAgentError {
    fn from(err: async_nats::header::ParseHeaderValueError) -> Self {
        Self::service_failed("header parsing", &err.to_string())
    }
}

impl From<nats_utils::types::ServiceError> for HostAgentError {
    fn from(err: nats_utils::types::ServiceError) -> Self {
        Self::Service {
            operation: "NATS service".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<async_nats::Error> for HostAgentError {
    fn from(err: async_nats::Error) -> Self {
        Self::Remote(RemoteError::Connection {
            endpoint: "NATS connection".to_string(),
            reason: err.to_string(),
        })
    }
}

impl From<async_nats::error::Error<async_nats::ConnectErrorKind>> for HostAgentError {
    fn from(err: async_nats::error::Error<async_nats::ConnectErrorKind>) -> Self {
        Self::Remote(RemoteError::Connection {
            endpoint: "NATS connection".to_string(),
            reason: err.to_string(),
        })
    }
}

impl From<DrainError> for HostAgentError {
    fn from(err: DrainError) -> Self {
        Self::service_failed("NATS client drain", &err.to_string())
    }
}

impl From<nkeys::error::Error> for HostAgentError {
    fn from(err: nkeys::error::Error) -> Self {
        Self::Crypto {
            operation: "NATS key operation".to_string(),
            source: err,
        }
    }
}

// Custom event errors
impl From<url::ParseError> for HostAgentError {
    fn from(err: url::ParseError) -> Self {
        Self::Service {
            operation: "URL parsing".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<tempfile::PersistError> for HostAgentError {
    fn from(err: tempfile::PersistError) -> Self {
        Self::Service {
            operation: "temporary file persistence".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<chrono::OutOfRangeError> for HostAgentError {
    fn from(err: chrono::OutOfRangeError) -> Self {
        Self::Service {
            operation: "time duration conversion".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<bson::oid::Error> for HostAgentError {
    fn from(err: bson::oid::Error) -> Self {
        Self::service_failed("BSON ObjectId parsing", &err.to_string())
    }
}

impl From<serde_json::Error> for HostAgentError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            operation: "JSON serialization".to_string(),
            source: err,
        }
    }
}

impl From<workload::types::WorkloadError> for HostAgentError {
    fn from(err: workload::types::WorkloadError) -> Self {
        Self::Workload {
            operation: "workload operation".to_string(),
            source: err,
        }
    }
}

impl From<anyhow::Error> for HostAgentError {
    fn from(err: anyhow::Error) -> Self {
        Self::System {
            operation: "anyhow error".to_string(),
            source: err,
        }
    }
}

// Error context trait for adding context to results
pub trait ErrorContext<T> {
    fn with_context(self, context: &str) -> HostAgentResult<T>;
    fn with_context_fn<F>(self, context_fn: F) -> HostAgentResult<T>
    where
        F: FnOnce() -> String;
}

impl<T> ErrorContext<T> for Result<T, HostAgentError> {
    fn with_context(self, context: &str) -> HostAgentResult<T> {
        self.map_err(|e| e.with_context(context))
    }

    fn with_context_fn<F>(self, context_fn: F) -> HostAgentResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| e.with_context(&context_fn()))
    }
}

// Extension trait for common error patterns
pub trait ResultExt<T, E> {
    fn map_err_with_context(self, context: &str) -> Result<T, HostAgentError>
    where
        E: Into<HostAgentError>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: Into<HostAgentError>,
{
    fn map_err_with_context(self, context: &str) -> Result<T, HostAgentError> {
        self.map_err(|e| e.into().with_context(context))
    }
}
