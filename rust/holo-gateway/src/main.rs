use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use lazy_static::lazy_static;
use log::info;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;

mod types;

lazy_static! {
    static ref NODE_ID: String = {
        let n = std::env::var("NODE_ID")
            .expect("Need NODE_ID environment variable set to a unique UUID");
        n.to_string()
    };
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Listen Address
    let listen_addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    let listener = TcpListener::bind(listen_addr).await?;

    info!(
        "Listening on 0.0.0.0:8000 as node ID: {}",
        NODE_ID.to_string()
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(http_handler))
                .with_upgrades()
                .await
            {
                info!("Failed to service connection: {:?}", err);
            }
        });
    }
}

/// Handler for incoming HTTP requests.
async fn http_handler(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let onward_request: types::ForwardedHTTPRequest =
        types::ForwardedHTTPRequest::from_hyper(&req, &NODE_ID);
    info!("Request: {:?}", onward_request);
    // At this point, we ought to be able to forward onward_request over NATS to the agent and have
    // it pass it through to the HC gateway. The request should have broken down all of the HC
    // super protocol stuff layered over HTTP to allow us to easily route to the right holoports.

    // Here's where we'd take the response back from the HC gateway (via NATS) and return it back
    // to the client.
    Ok(Response::new(Full::new(Bytes::from("Gateway Response"))))
}
