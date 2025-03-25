pub mod nats_client;
pub mod routes;
pub mod types;

use anyhow::Result;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use types::http as http_types;

pub async fn run() -> Result<()> {
    // Listen Address
    let holo_gw_port = 8000;
    let listen_addr = SocketAddr::from(([0, 0, 0, 0], holo_gw_port));
    let listener = TcpListener::bind(listen_addr).await?;

    log::info!(
        "Listening on 0.0.0.0:{holo_gw_port} as node ID: {}",
        http_types::NODE_ID.as_str()
    );

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(routes::http_router))
                .with_upgrades()
                .await
            {
                log::error!("Failed to service connection: {err:?}");
            }
        });
    }
}
