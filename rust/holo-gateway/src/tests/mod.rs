use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

pub mod gateway;
pub mod routes;

pub struct TestHttpServer {
    addr: SocketAddr,
    _server_task: Arc<tokio::task::JoinHandle<()>>,
}

impl TestHttpServer {
    pub async fn new() -> Result<Self> {
        // Bind to a random port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        // Spawn server task
        let server_task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let io = TokioIo::new(stream);

                        tokio::task::spawn(async move {
                            if let Err(err) = http1::Builder::new()
                                .preserve_header_case(true)
                                .title_case_headers(true)
                                .serve_connection(io, service_fn(handle_request))
                                .with_upgrades()
                                .await
                            {
                                eprintln!("Error serving connection: {:?}", err);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {:?}", e);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            addr,
            _server_task: Arc::new(server_task),
        })
    }

    pub fn address(&self) -> String {
        format!("http://{}", self.addr)
    }
}

async fn handle_request(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    match crate::routes::http_router(req).await {
        Ok(response) => Ok(response),
        Err(e) => {
            let (status, body) = e.into_status_code_and_body();
            Ok(Response::builder()
                .status(status)
                .body(Full::new(body.into()))
                .unwrap())
        }
    }
}
