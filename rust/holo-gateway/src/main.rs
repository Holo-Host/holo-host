use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1::Builder;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use lazy_static::lazy_static;
use log::{debug, info};
use rustls::ServerConfig;
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use std::convert::Infallible;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

mod types;

lazy_static! {
    static ref NODE_ID: String = {
        let n = std::env::var("NODE_ID")
            .expect("Need NODE_ID environment variable set to a unique UUID");
        n.to_string()
    };
}

/// The name of the environment variable to override the default TCP port number.
const TCP_PORT_VAR_NAME: &str = "TCP_PORT";
/// The default TCP port to bind to.
const DEFAULT_TCP_PORT: u16 = 80;

/// The name of the environment variable to override the default certificate filename
const TLS_CERT_VAR_NAME: &str = "TLS_CERT";
/// A default filename for our public, signed certificate
const DEFAULT_CERT_FILE: &str = "cert.pem";

/// The name of the environment variable to override the default private key filename
const TLS_KEY_VAR_NAME: &str = "TLS_PRIVATE_KEY";
/// A default filename for our private key
const DEFAULT_KEY_FILE: &str = "private.key";

/// Wrapper to load TLS certificate file(s)
fn load_certs(
    filename: String,
) -> Result<Vec<CertificateDer<'static>>, Box<dyn std::error::Error>> {
    let mut ret: Vec<CertificateDer<'static>> = vec![];
    for cert in CertificateDer::pem_file_iter(filename).unwrap() {
        ret.push(cert?);
    }
    Ok(ret)
}

/// Wrapper to load TLS private key
fn load_private_key(
    filename: String,
) -> Result<PrivateKeyDer<'static>, Box<dyn std::error::Error>> {
    let key = PrivateKeyDer::from_pem_file(filename)?;

    Ok(key)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Process-wide crypto providers
    let _ = rustls::crypto::ring::default_provider().install_default();

    let tcp_port = match env::var(TCP_PORT_VAR_NAME) {
        Ok(port) => port.parse().unwrap(),
        Err(_) => DEFAULT_TCP_PORT,
    };

    // Listen Address
    let listen_addr = SocketAddr::from(([0, 0, 0, 0], tcp_port));
    let listener = TcpListener::bind(listen_addr).await?;

    info!(
        "Listening on 0.0.0.0:{} as node ID: {}",
        tcp_port,
        NODE_ID.to_string()
    );

    // Pick up selected private key if specified, or try a default path/filename
    let key = match env::var(TLS_KEY_VAR_NAME) {
        Ok(path) => load_private_key(path)?,
        Err(_) => load_private_key(DEFAULT_KEY_FILE.to_string())?,
    };
    // Same for the signed certificate
    let certs = match env::var(TLS_CERT_VAR_NAME) {
        Ok(path) => load_certs(path)?,
        Err(_) => load_certs(DEFAULT_CERT_FILE.to_string())?,
    };

    // Add a TLS handler
    let mut tls_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("Failed to load certificates and start TLS: {}", e))?;
    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    // Handler for HTTP request after TLS does its thing
    let service_handler = service_fn(http_handler);

    loop {
        let (stream, remote) = listener.accept().await?;
        // Later we'll include things like this in access logs
        debug!("Accepted connection from {}", remote);

        let acceptor = acceptor.clone();

        tokio::task::spawn(async move {
            // Handle the TLS handshake
            let tls_stream = match acceptor.accept(stream).await {
                Ok(tls_stream) => tls_stream,
                Err(e) => {
                    info!("Failed to perform TLS handshake: {}", e);
                    return;
                }
            };

            if let Err(err) = Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(TokioIo::new(tls_stream), service_handler)
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
