pub mod gateway;

use std::sync::Arc;

use crate::types::error::HoloHttpGatewayError;
use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, Request, Response, StatusCode};
use nats_utils::jetstream_client::JsClient;
use uuid::Uuid;

pub async fn http_router(
    node_id: &Uuid,
    nats_client: Arc<JsClient>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, HoloHttpGatewayError> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => health_check(nats_client).await,
        (&Method::GET, _) => gateway::run(node_id, nats_client, req).await,
        (method, route) => not_found(method, route),
    }
}

pub async fn health_check(
    nats_client: Arc<JsClient>,
) -> Result<Response<Full<Bytes>>, HoloHttpGatewayError> {
    let state = match nats_client.check_connection().await {
        Ok(state) => state.to_string(),
        Err(e) => e.to_string(),
    };

    Ok(Response::new(Full::new(state.into())))
}

pub fn not_found(
    method: &Method,
    route: &str,
) -> Result<Response<Full<Bytes>>, HoloHttpGatewayError> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::new(
            format!("Found unrecognized method and route combination on Holo Gateway: {method} @ {route}").into(),
        ))
        .map_err(|e| {
            HoloHttpGatewayError::Internal(format!(
                "Failed to return 404 route not found. err={e:?}"
            ))
        })
}
