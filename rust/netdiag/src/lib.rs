use clap::ValueEnum;
use log::{debug, info};
use rustdns::types::*;
use rustdns::Message;
use rustls::RootCertStore;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum NetDiagError {
    #[error("I/O Error")]
    IOError(#[from] std::io::Error),
    #[error("TLS Error")]
    TLSError(#[from] rustls::Error),
}

#[derive(Debug)]
pub struct RequestStats {
    /// A text label
    pub id: String,
    /// Stats for individual phases in the request
    pub stats: Vec<PhaseStat>,
    /// Global start time across all phases
    pub start: Instant,
    /// Duration for whole request, including all phases (successful or otherwise)
    pub elapsed: Duration,
    /// This is the result of the last phase of the request (ie, the one that failed, if any).
    pub result: Result<(), NetDiagError>,
}

impl RequestStats {
    fn new(id: String) -> Self {
        RequestStats {
            id,
            stats: vec![],
            start: Instant::now(),
            elapsed: Duration::new(0, 0),
            result: Ok(()),
        }
    }

    fn start_phase(&mut self, phase: PhaseType) {
        self.stats.push(PhaseStat::start(phase));
    }

    fn phase_err(&mut self, error: String) {
        self.stats
            .last_mut()
            .expect("No stats phase open.")
            .phase_err(error);
    }

    fn end_phase(&mut self) {
        // make sure that all stats are stopped.
        for stat in self.stats.iter_mut() {
            stat.stop();
        }
    }

    fn finalise(&mut self) {
        self.elapsed = self.start.elapsed();
    }
}

impl fmt::Display for RequestStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: tot:{} ", self.id, self.elapsed.as_nanos())?;
        for phase in self.stats.iter() {
            write!(f, "{} ", phase)?;
        }
        write!(f, "nanoseconds")
    }
}

#[derive(Debug)]
pub struct PhaseStat {
    pub phase: PhaseType,
    start: Instant,
    pub elapsed: Option<Duration>,
    pub error: Option<String>,
}

impl PhaseStat {
    fn start(phase: PhaseType) -> Self {
        PhaseStat {
            phase,
            start: Instant::now(),
            elapsed: None,
            error: None,
        }
    }

    fn phase_err(&mut self, error: String) {
        if self.elapsed.is_none() {
            self.elapsed = Some(self.start.elapsed());
        }
        self.error = Some(error);
    }

    fn stop(&mut self) {
        // Set the elapsed time the first time that we're called
        if self.elapsed.is_none() {
            self.elapsed = Some(self.start.elapsed());
        }
    }
}

impl fmt::Display for PhaseStat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.phase,
            self.elapsed.unwrap_or_default().as_nanos()
        )
    }
}

#[derive(Debug)]
pub enum PhaseType {
    NameResolution,
    TlsSession,
    TcpConnect,
    SendRequest,
    RecvResponse,
}

impl fmt::Display for PhaseType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NameResolution => write!(f, "name-resolution"),
            Self::TcpConnect => write!(f, "tcp-connect"),
            Self::TlsSession => write!(f, "tls-session"),
            Self::SendRequest => write!(f, "send-request"),
            Self::RecvResponse => write!(f, "receive-response"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetDiagConfig {
    pub global: GlobalConfig,
    pub queries: Vec<QueryDefinition>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub ip_version: IPVersion,
}
#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum)]
pub enum IPVersion {
    IPV4,
    IPV6,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryMethod {
    HTTP { path: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDefinition {
    pub nameserver: String,
    pub use_tls: bool,
    pub hostname: String,
    pub port: u16,
    pub method: QueryMethod,
}

pub fn resolve_name(
    cfg: &QueryDefinition,
    ip_ver: &IPVersion,
) -> std::result::Result<String, std::io::Error> {
    let mut m = Message::default();

    match ip_ver {
        IPVersion::IPV4 => m.add_question(&cfg.hostname, Type::A, Class::Internet),
        IPVersion::IPV6 => m.add_question(&cfg.hostname, Type::AAAA, Class::Internet),
    }

    // local end of the UDP socket. TODO: this should support IPv6 too.
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::new(5, 0)))?;
    socket.connect(&cfg.nameserver)?;

    let dns_query = m.to_vec()?;
    socket.send(&dns_query)?;

    let mut resp = [0; 4096];
    let len = socket.recv(&mut resp)?;

    let response = Message::from_slice(&resp[0..len])?;

    debug!("DNS response: {:?}", &response);
    let mut ret: String = "".to_string();
    if response.rcode == Rcode::NoError {
        // We only take the first record for now, but could potentially loop through multiple where multiple
        // are returned.
        let record = &response.answers[0];
        match record.resource {
            rustdns::Resource::A(ip) => ret = ip.to_string(),
            rustdns::Resource::AAAA(ip) => ret = ip.to_string(),
            _ => panic!("Got wrong record back"),
        }
    }

    Ok(ret)
}

pub fn do_requests(cfg: &NetDiagConfig) -> Vec<RequestStats> {
    let mut ret: Vec<RequestStats> = vec![];
    for query in &cfg.queries {
        debug!(
            "Querying: {}:{} via DNS server {}",
            query.hostname, query.port, query.nameserver
        );
        let req = do_request(query, &cfg.global.ip_version);
        ret.push(req);
    }
    ret
}

/// Perform a HTTPS request and everything involved in making that happen, including DNS, TLS, TCP,
/// etc. This will always return a [RequestStats] object and not an error. If an error occurs, it
/// is included inside the RequestStats object.
pub fn do_request(cfg: &QueryDefinition, ip_ver: &IPVersion) -> RequestStats {
    // Create RequestStats object to track timing/errors
    let mut stats = RequestStats::new(Uuid::new_v4().to_string());

    // Perform name resolution
    stats.start_phase(PhaseType::NameResolution);
    let result = resolve_name(cfg, ip_ver);
    let ip = match result {
        Ok(addr) => addr,
        Err(e) => {
            stats.phase_err(e.to_string());
            stats.result = Err(NetDiagError::IOError(e));
            stats.finalise();
            return stats;
        }
    };
    stats.end_phase();

    let host = format!("{}:{}", ip, cfg.port);
    stats.start_phase(PhaseType::TcpConnect);
    let result = TcpStream::connect(host);
    let mut sock = match result {
        Ok(sock) => sock,
        Err(e) => {
            stats.phase_err(e.to_string());
            stats.result = Err(NetDiagError::IOError(e));
            stats.finalise();
            return stats;
        }
    };
    stats.end_phase();

    // Different methods fill this in to form the request we'll make over the socket. String is
    // fine for things like HTTP.
    let request: String = match &cfg.method {
        QueryMethod::HTTP { path: method } => {
            info!("HTTP request for {}", method);
            let mut req = format!("GET {} HTTP/1.1\r\n", &method);
            // Set headers here. Host: header is required for HTTP 1.1.
            // TODO: In future, we could/should also include support for using our host key(s) to
            // authenticate against the server as a test.
            let headers: Vec<(&str, &str)> = vec![
                ("Host", cfg.hostname.as_str()),
                ("User-Agent", "Holo-Netdiag"),
                ("Accept", "*/*"),
                ("Connection", "close"),
            ];
            for (k, v) in headers.iter() {
                req.push_str(format!("{}: {}\r\n", k, v).as_str());
            }
            req.push_str("\r\n");
            req
        }
    };

    match cfg.use_tls {
        true => {
            // Start TLS handshake
            stats.start_phase(PhaseType::TlsSession);
            let root_store = RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.into(),
            };

            let _ = rustls::crypto::ring::default_provider().install_default();

            let mut tls_config = rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();

            tls_config.key_log = Arc::new(rustls::KeyLogFile::new());

            let tls_server = cfg
                .hostname
                .clone()
                .try_into()
                .expect("Can't parse hostname for TLS");
            let result = rustls::ClientConnection::new(Arc::new(tls_config), tls_server);
            let mut tls_sock = match result {
                Ok(t) => t,
                Err(e) => {
                    stats.phase_err(e.to_string());
                    stats.result = Err(NetDiagError::TLSError(e));
                    stats.finalise();
                    return stats;
                }
            };
            // This creates a stream around our TCP socket using our TLS handle.
            let mut tls = rustls::Stream::new(&mut tls_sock, &mut sock);
            stats.end_phase();

            // Send request over TLS
            stats.start_phase(PhaseType::SendRequest);
            match tls.write_all(request.as_bytes()) {
                Ok(_) => {}
                Err(e) => {
                    stats.phase_err(e.to_string());
                    stats.result = Err(NetDiagError::IOError(e));
                    stats.finalise();
                    return stats;
                }
            }
            stats.end_phase();

            // Read response
            stats.start_phase(PhaseType::RecvResponse);
            let mut plaintext = Vec::new();
            match tls.read_to_end(&mut plaintext) {
                Ok(_) => {}
                Err(e) => {
                    stats.phase_err(e.to_string());
                    stats.result = Err(NetDiagError::IOError(e));
                    stats.finalise();
                    return stats;
                }
            }
            stats.end_phase();
            debug!("Read {} bytes from plaintext response.", plaintext.len());
            // We don't actually care about the content. Just the length. We may validate the data
            // later. It's a vector uf UTF8 bytes.
            // stdout().write_all(&plaintext).unwrap();
        }
        false => {
            // start send request
            stats.start_phase(PhaseType::SendRequest);
            let count = match sock.write(request.as_bytes()) {
                Ok(c) => c,
                Err(e) => {
                    stats.phase_err(e.to_string());
                    stats.result = Err(NetDiagError::IOError(e));
                    stats.finalise();
                    return stats;
                }
            };
            stats.end_phase();

            // XXX: read response

            debug!("Wrote {} bytes", count);
        }
    }

    stats.finalise();

    stats
}
