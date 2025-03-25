mod gateway;

use crate::types::http::HoloHttpGatewayError;
use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, Request, Response, StatusCode};

pub async fn http_router(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, HoloHttpGatewayError> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => gateway::run(req).await,
        (&Method::GET, "/health") => health_check(),
        _ => not_found(),
    }
}

pub fn health_check() -> Result<Response<Full<Bytes>>, HoloHttpGatewayError> {
    Ok(Response::new(Full::new("Ok".into())))
}

pub fn not_found() -> Result<Response<Full<Bytes>>, HoloHttpGatewayError> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(
            "Found unrecognized route on Holo Gateway".as_bytes().into(),
        ))
        .map_err(|e| {
            HoloHttpGatewayError::Internal(format!(
                "Failed to return 404 route not found. err={e:?}"
            ))
        })
}
