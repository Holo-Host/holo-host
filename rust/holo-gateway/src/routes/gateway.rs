use crate::types::{
    error::HoloHttpGatewayError,
    http::{self as http_types, ForwardedHTTPRequest, NODE_ID, SuperProtocol},
    nats as nats_types,
};
use bytes::Bytes;
use futures::StreamExt;
use http_body_util::Full;
use hyper::{Request, Response, body::Body};
use std::fmt::Debug;
use tokio::time::{self, Duration};

pub async fn run<B>(req: Request<B>) -> Result<Response<Full<Bytes>>, HoloHttpGatewayError>
where
    B: Body<Data = Bytes> + Send + Sync + 'static + Debug,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let onward_request = ForwardedHTTPRequest::from_hyper(&req, &NODE_ID);
    let request_headers = onward_request.headers.clone();
    log::info!("Created the Holo Gateway Forward Request. request={onward_request:?}");

    // At this point, we ought to be able to forward onward_request over NATS to the agent and have
    // it pass it through to the HC gateway. The request should have broken down all of the HC
    // super protocol stuff layered over HTTP to allow us to easily route to the right holoports.
    if let Some(protocol) = onward_request.super_proto.clone() {
        match protocol {
            SuperProtocol::HolochainHTTP(hc_payload) => {
                let payload_bytes = serde_json::to_vec(&hc_payload)?;
                let holo_gatway_subject = hc_payload.into_gateway_request_subject();
                log::debug!(
                    "About to send out a Holochain Gateway request via nats. subject={holo_gatway_subject}"
                );

                let nats_client = nats_types::get_nats_client().await;
                nats_client
                    .publish_with_headers(
                        holo_gatway_subject.clone(),
                        onward_request.headers,
                        payload_bytes.into(),
                    )
                    .await?;

                let mut subscription = nats_client
                    .subscribe(hc_payload.into_gateway_reply_subject())
                    .await?;

                let timeout_duration = Duration::from_secs(180);
                let response = time::timeout(timeout_duration, subscription.next()).await;

                match response {
                    Ok(Some(nats_gateway_response)) => {
                        log::debug!("HC Gateway Response via Nats: {nats_gateway_response:?}");

                        // Build response *with headers* from the forwarded request
                        let mut response_builder = Response::builder();
                        for (key, value) in
                            http_types::try_add_hyper_headers(&request_headers)?.iter()
                        {
                            response_builder = response_builder.header(key, value);
                        }

                        return Ok(response_builder
                            .status(200)
                            .body(Full::new(nats_gateway_response.payload))
                            .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()))));
                    }
                    Ok(None) => {
                        return Err(HoloHttpGatewayError::Nats(
                            "NATS subscription closed before receiving a message".to_string(),
                        ));
                    }
                    Err(_) => {
                        return Err(HoloHttpGatewayError::Internal(
                            "Timed out waiting for NATS response".to_string(),
                        ));
                    }
                }
            }
        }
    };

    Err(HoloHttpGatewayError::BadRequest(
        "Failed to locate required params for HoloHttpGateway request.".to_string(),
    ))
}
