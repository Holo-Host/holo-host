#[derive(Debug, thiserror::Error)]
pub enum RemoteError {
    #[error("Connection failed to {endpoint}: {reason}")]
    Connection { endpoint: String, reason: String },

    #[error("Remote operation failed: {operation} - {reason}")]
    Operation { operation: String, reason: String },
}

impl RemoteError {
    pub fn operation_failed(operation: &str, reason: &str) -> Self {
        Self::Operation {
            operation: operation.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn connection_failed(endpoint: &str, reason: &str) -> Self {
        Self::Connection {
            endpoint: endpoint.to_string(),
            reason: reason.to_string(),
        }
    }
}

impl From<serde_json::Error> for RemoteError {
    fn from(err: serde_json::Error) -> Self {
        Self::Operation {
            operation: "JSON serialization".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<anyhow::Error> for RemoteError {
    fn from(err: anyhow::Error) -> Self {
        Self::Operation {
            operation: "remote operation".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<std::io::Error> for RemoteError {
    fn from(err: std::io::Error) -> Self {
        Self::Operation {
            operation: "I/O operation".to_string(),
            reason: err.to_string(),
        }
    }
}

impl From<async_nats::SubscribeError> for RemoteError {
    fn from(err: async_nats::SubscribeError) -> Self {
        Self::Connection {
            endpoint: "NATS subscription".to_string(),
            reason: err.to_string(),
        }
    }
}

pub type RemoteResult<T> = Result<T, RemoteError>;
