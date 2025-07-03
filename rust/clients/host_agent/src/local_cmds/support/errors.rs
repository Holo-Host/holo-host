#[derive(Debug, thiserror::Error)]
pub enum SupportError {
    #[error("Diagnostic operation failed: {operation}: {reason}")]
    Diagnostic { operation: String, reason: String },
}

impl SupportError {
    pub fn diagnostic_failed(operation: &str, reason: &str) -> Self {
        Self::Diagnostic {
            operation: operation.to_string(),
            reason: reason.to_string(),
        }
    }
}

pub type SupportResult<T> = Result<T, SupportError>;
