use thiserror::Error;
#[derive(Error, Debug)]

pub enum OrchestratorError {
    #[error("Database connection failed: {0}")]
    Database(#[from] mongodb::error::Error),
    
    #[error("NATS connection failed: {0}")]
    Nats(#[from] async_nats::Error),
    
    #[error("Client failure: {0}")]
    Client(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Shutdown error: {0}")]
    Shutdown(String),
}