use crate::types::{
    error::HoloHttpGatewayError,
    http::{ForwardedHTTPRequest, SuperProtocol},
};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Body, Request, Response};
use nats_utils::jetstream_client::JsClient;
use std::{fmt::Debug, sync::Arc};
use tokio::time::{self, Duration};
use uuid::Uuid;

pub async fn run<B>(
    node_id: &Uuid,
    nats_client: Arc<JsClient>,
    req: Request<B>,
) -> Result<Response<Full<Bytes>>, HoloHttpGatewayError>
where
    B: Body<Data = Bytes> + Send + Sync + 'static + Debug,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let onward_request = ForwardedHTTPRequest::from_hyper(&req, node_id);
    let request_headers = onward_request.headers.clone();
    log::info!("Created the Holo Gateway Forward Request. request={onward_request:?}");

    // At this point, we ought to be able to forward onward_request over NATS to the agent and have
    // it pass it through to the HC gateway. The request should have broken down all of the HC
    // super protocol stuff layered over HTTP to allow us to easily route to the right holoports.
    if let Some(protocol) = onward_request.super_proto.clone() {
        match protocol {
            SuperProtocol::HolochainHTTP(hc_payload) => {
                log::debug!("About to send out a Holochain Gateway request via nats: client={nats_client:?}, request_payload={hc_payload:?}, request_headers={request_headers:?}");

                let timeout_duration = Duration::from_secs(10);
                let response = time::timeout(
                    timeout_duration,
                    nats_utils::types::hc_http_gw_nats_request(
                        nats_client,
                        hc_payload,
                        request_headers,
                    ),
                )
                .await;

                match response {
                    Ok(Ok(nats_gateway_response)) => {
                        log::debug!("HC Gateway Response via Nats: {nats_gateway_response:?}");

                        // Build response *with headers* from the request received via NATS
                        let mut response_builder = Response::builder();
                        for (key, value) in nats_gateway_response.response_headers.iter() {
                            response_builder = response_builder.header(key, value.to_vec());
                        }

                        match response_builder
                            .status(200)
                            .body(Full::new(nats_gateway_response.response_bytes))
                        {
                            Ok(response) => Ok(response),
                            Err(e) => Err(HoloHttpGatewayError::Internal(format!(
                                "error assembling response: {e}"
                            ))),
                        }
                    }
                    Ok(Err(e)) => Err(HoloHttpGatewayError::Nats(format!(
                        "NATS subscription closed before receiving a message: {e}",
                    ))),
                    Err(_) => Err(HoloHttpGatewayError::Internal(
                        "Timed out waiting for NATS response".to_string(),
                    )),
                }
            }
        }
    } else {
        Err(HoloHttpGatewayError::BadRequest(
            "Failed to locate required params for HoloHttpGateway request.".to_string(),
        ))
    }
}
