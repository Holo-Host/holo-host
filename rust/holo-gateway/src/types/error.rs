use holochain_http_gateway::HcHttpGatewayError;
use hyper::StatusCode;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum HoloHttpGatewayError {
    Holochain(HcHttpGatewayError),
    BadRequest(String),
    Nats(String),
    Internal(String),
}
impl HoloHttpGatewayError {
    pub fn into_status_code_and_body(self) -> (StatusCode, String) {
        match self {
            HoloHttpGatewayError::Holochain(e) => e.into_status_code_and_body(),
            HoloHttpGatewayError::BadRequest(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            HoloHttpGatewayError::Nats(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            HoloHttpGatewayError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    }
}
impl fmt::Display for HoloHttpGatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl From<async_nats::SubscribeError> for HoloHttpGatewayError {
    fn from(value: async_nats::SubscribeError) -> Self {
        Self::Nats(value.to_string())
    }
}
impl From<async_nats::error::Error<async_nats::client::PublishErrorKind>> for HoloHttpGatewayError {
    fn from(value: async_nats::error::Error<async_nats::client::PublishErrorKind>) -> Self {
        Self::Nats(value.to_string())
    }
}
impl From<async_nats::header::ParseHeaderNameError> for HoloHttpGatewayError {
    fn from(value: async_nats::header::ParseHeaderNameError) -> Self {
        Self::Nats(value.to_string())
    }
}
impl From<async_nats::header::ParseHeaderValueError> for HoloHttpGatewayError {
    fn from(value: async_nats::header::ParseHeaderValueError) -> Self {
        Self::Nats(value.to_string())
    }
}
impl From<std::string::FromUtf8Error> for HoloHttpGatewayError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Internal(value.to_string())
    }
}
impl From<serde_json::Error> for HoloHttpGatewayError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(value.to_string())
    }
}
impl From<hyper::header::InvalidHeaderValue> for HoloHttpGatewayError {
    fn from(value: hyper::header::InvalidHeaderValue) -> Self {
        Self::BadRequest(value.to_string())
    }
}
impl From<hyper::header::InvalidHeaderName> for HoloHttpGatewayError {
    fn from(value: hyper::header::InvalidHeaderName) -> Self {
        Self::BadRequest(value.to_string())
    }
}

impl Error for HoloHttpGatewayError {}
