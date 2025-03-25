use crate::types::{
    http::{self as http_types, SuperProtocol},
    nats as nats_types,
};
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response};

pub async fn run(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, http_types::HoloHttpGatewayError> {
    let onward_request = http_types::ForwardedHTTPRequest::from_hyper(&req, &http_types::NODE_ID);
    log::info!("Request: {:?}", onward_request);

    // At this point, we ought to be able to forward onward_request over NATS to the agent and have
    // it pass it through to the HC gateway. The request should have broken down all of the HC
    // super protocol stuff layered over HTTP to allow us to easily route to the right holoports.
    if let Some(protocol) = onward_request.super_proto {
        match protocol {
            SuperProtocol::HolochainHTTP(hc_payload) => {
                let headers = onward_request.headers.into();

                let payload_bytes = serde_json::to_vec(&hc_payload)?;
                let holo_gatway_subject = hc_payload.into_subject();
                println!("About to send out the {holo_gatway_subject}");

                let nats_client = nats_types::get_nats_client().await;
                let nats_gateway_response = nats_client
                    .request_with_headers(holo_gatway_subject, headers, payload_bytes.into())
                    .await?;

                return Ok(Response::new(Full::new(nats_gateway_response.payload)));
            }
            _ => {
                return Err(http_types::HoloHttpGatewayError::BadRequest(
                    "Holochain protocol params not found.".to_string(),
                ));
            }
        }
    };

    Err(http_types::HoloHttpGatewayError::BadRequest(
        "No protocol params found.".to_string(),
    ))
}
