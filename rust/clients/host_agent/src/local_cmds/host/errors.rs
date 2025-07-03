use crate::local_cmds::support::errors::SupportError;
use crate::remote_cmds::errors::RemoteError;
use async_nats::client::DrainError;

/// Main application error type that handles all possible errors
pub type HostAgentResult<T> = Result<T, HostAgentError>;

#[derive(Debug, thiserror::Error)]
pub enum HostAgentError {
    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Authentication failed: {reason}")]
    Authentication { reason: String },

    #[error("Configuration validation failed: {reason}")]
    Validation { reason: String },

    #[error("Service operation failed: {operation} - {reason}")]
    Service { operation: String, reason: String },

    #[error("Service unavailable: {service} - {reason}")]
    ServiceUnavailable { service: String, reason: String },

    #[error("System information unavailable: {component}")]
    SystemInfo { component: String },

    #[error("System operation failed: {0}")]
    System(#[from] anyhow::Error),

    #[error("Support operation failed: {0}")]
    Support(#[from] SupportError),

    #[error("Remote operation failed: {0}")]
    Remote(#[from] RemoteError),
}

impl HostAgentError {
    // Configuration errors
    pub fn config(message: &str) -> Self {
        Self::Config {
            message: message.to_string(),
        }
    }

    // Authentication errors
    pub fn auth_failed(reason: &str) -> Self {
        Self::Authentication {
            reason: reason.to_string(),
        }
    }

    // Service availability errors
    pub fn service_unavailable(service: &str, reason: &str) -> Self {
        Self::ServiceUnavailable {
            service: service.to_string(),
            reason: reason.to_string(),
        }
    }

    // System information errors
    pub fn system_info_unavailable(component: &str) -> Self {
        Self::SystemInfo {
            component: component.to_string(),
        }
    }

    // Validation errors
    pub fn validation(reason: &str) -> Self {
        Self::Validation {
            reason: reason.to_string(),
        }
    }

    // Service operation errors
    pub fn service_failed(operation: &str, reason: &str) -> Self {
        Self::Service {
            operation: operation.to_string(),
            reason: reason.to_string(),
        }
    }
}

// Conversion to authentication crate's AuthError for signature operations
impl From<HostAgentError> for authentication::types::AuthError {
    fn from(err: HostAgentError) -> Self {
        match err {
            HostAgentError::Authentication { reason } => {
                authentication::types::AuthError::auth_failed(&reason)
            }
            HostAgentError::Config { message } => {
                authentication::types::AuthError::config_error(&message)
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
            _ => workload::types::WorkloadError::service_error(&err.to_string()),
        }
    }
}

// From implementations - replaces manual map_err usage across the crate
// Standard lib errors
impl From<std::io::Error> for HostAgentError {
    fn from(err: std::io::Error) -> Self {
        Self::service_failed("I/O operation", &err.to_string())
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
                Self::auth_failed("Authentication request timed out")
            }
            _ => Self::auth_failed(&format!("Authentication request failed: {}", err)),
        }
    }
}

impl From<async_nats::PublishError> for HostAgentError {
    fn from(err: async_nats::PublishError) -> Self {
        Self::auth_failed(&format!("Failed to publish message: {}", err))
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
        Self::Service {
            operation: "NATS key operation".to_string(),
            reason: err.to_string(),
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
        Self::Service {
            operation: "JSON serialization".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<workload::types::WorkloadError> for HostAgentError {
    fn from(err: workload::types::WorkloadError) -> Self {
        Self::service_failed("workload operation", &err.to_string())
    }
}
