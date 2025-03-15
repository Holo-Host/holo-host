use super::{
    jetstream_service::JsStreamService,
    types::{
        Credentials, ErrClientDisconnected, EventHandler, EventListener, JsClientBuilder,
        JsServiceBuilder, PublishInfo,
    },
};
use anyhow::{Context, Result};
use async_nats::{jetstream, ServerAddr, ServerInfo};
use core::option::Option::None;
use std::sync::Arc;
use std::time::{Duration, Instant};

impl std::fmt::Debug for JsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsClient")
            .field("url", &self.url)
            .field("name", &self.name)
            .field("client", &self.client)
            .field("js_context", &self.js_context)
            .field("js_services", &self.js_services)
            .field("service_log_prefix", &self.service_log_prefix)
            .finish()
    }
}

#[derive(Clone)]
pub struct JsClient {
    url: ServerAddr,
    pub name: String,
    on_msg_published_event: Option<EventHandler>,
    on_msg_failed_event: Option<EventHandler>,
    pub js_services: Option<Vec<JsStreamService>>,
    pub js_context: jetstream::Context,
    service_log_prefix: String,
    client: async_nats::Client, // Built-in Nats Client which manages the cloned clients within jetstream contexts
}

/// Implements a permissive `ServerCertVerifier` for convenience in testing.
pub mod tls_skip_verifier {
    use std::sync::Arc;

    use async_nats::rustls::{
        self,
        client::danger::{HandshakeSignatureValid, ServerCertVerified},
    };

    /// this needs to run early in the process or else it might conflict
    /// with something else installing a crypto provider
    pub fn early_in_process_install_crypto_provider() -> bool {
        if let Err(other) = async_nats::rustls::crypto::ring::default_provider().install_default() {
            log::error!("error installing the default ring crypto provider. custom cert verification logic may not work as expected.");
            let _ = <async_nats::rustls::crypto::CryptoProvider as Clone>::clone(&other)
                .install_default();
            false
        } else {
            true
        }
    }

    #[derive(Debug)]
    pub struct SkipServerVerification;

    impl SkipServerVerification {
        pub fn new() -> Arc<Self> {
            Arc::new(Self)
        }
    }

    use rustls::client::danger::ServerCertVerifier;
    impl ServerCertVerifier for SkipServerVerification {
        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &rustls::pki_types::CertificateDer<'_>,
            _dss: &rustls::DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            use rustls::SignatureScheme::*;

            vec![
                RSA_PKCS1_SHA1,
                ECDSA_SHA1_Legacy,
                RSA_PKCS1_SHA256,
                ECDSA_NISTP256_SHA256,
                RSA_PKCS1_SHA384,
                ECDSA_NISTP384_SHA384,
                RSA_PKCS1_SHA512,
                ECDSA_NISTP521_SHA512,
                RSA_PSS_SHA256,
                RSA_PSS_SHA384,
                RSA_PSS_SHA512,
                ED25519,
                ED448,
            ]
        }

        fn verify_server_cert(
            &self,
            _end_entity: &rustls::pki_types::CertificateDer<'_>,
            _intermediates: &[rustls::pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
    }
}

impl JsClient {
    pub async fn new(p: JsClientBuilder) -> Result<Self, async_nats::Error> {
        let JsClientBuilder { ref nats_url, .. } = p;

        let mut connect_options = async_nats::ConnectOptions::new()
            .name(&p.name)
            // required for websocket connections
            .reconnect_delay_callback({
                let nats_url = p.nats_url.clone();
                move |i| {
                    log::warn!("[{i}] problems connecting to {nats_url:?}");
                    Duration::from_secs(i as u64)
                }
            })
            .ping_interval(p.ping_interval.unwrap_or(Duration::from_secs(120)))
            .request_timeout(Some(p.request_timeout.unwrap_or(Duration::from_secs(1))))
            .custom_inbox_prefix(&p.inbox_prefix);

        if let (Some(user), Some(password_file)) = (p.maybe_nats_user, p.maybe_nats_password_file) {
            let pass = std::fs::read_to_string(&password_file)
                .context(format!("reading {password_file:?}"))?;

            connect_options = connect_options.user_and_password(user, pass);
        }

        if let Some(credentials_list) = p.credentials {
            for credentials in credentials_list {
                match credentials {
                    Credentials::Password(user, pw) => {
                        connect_options = connect_options.user_and_password(user, pw);
                    }
                    Credentials::Path(cp) => {
                        let path = std::path::Path::new(&cp);
                        connect_options = connect_options.credentials_file(path).await?;
                    }
                    Credentials::Token(t) => {
                        connect_options = connect_options.token(t);
                    }
                }
            }
        };

        if p.nats_skip_tls_verification_danger {
            log::warn!("! configuring TLS client to skip certificate verification. DO NOT RUN THIS IN PRODUCTION !");

            let tls_client = async_nats::rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(tls_skip_verifier::SkipServerVerification::new())
                .with_no_client_auth();

            connect_options = connect_options.tls_client_config(tls_client);
        }

        if let "wss" | "tls" = nats_url.as_ref().scheme() {
            log::info!("tls with handshake-first enabled.");
            connect_options = connect_options.tls_first();
        };

        let client = {
            let context_msg = format!(
                "connecting NATS to {nats_url:?} (websocket? {}) with options: {connect_options:?}",
                nats_url.is_websocket()
            );
            connect_options
                .connect(nats_url.as_ref())
                .await
                .context(context_msg)?
        };
        let service_log_prefix = format!("NATS-CLIENT-LOG::{}::", p.name);
        log::info!(
            "{service_log_prefix}Connected to NATS server at {:?}",
            *p.nats_url
        );

        let mut js_client = JsClient {
            url: p.nats_url.as_ref().clone(),
            name: p.name,
            on_msg_published_event: None,
            on_msg_failed_event: None,
            js_services: None,
            js_context: jetstream::new(client.clone()),
            service_log_prefix,
            client,
        };

        for listener in p.listeners {
            listener(&mut js_client);
        }

        Ok(js_client)
    }

    pub fn get_server_info(&self) -> ServerInfo {
        self.client.server_info()
    }

    pub async fn get_stream_info(
        &self,
        stream_name: &str,
    ) -> Result<jetstream::stream::Info, async_nats::Error> {
        let stream = &self.js_context.get_stream(stream_name).await?;
        let info = stream.get_info().await?;
        log::debug!(
            "{}JetStream info: stream:{stream_name}, info:{info:?}",
            self.service_log_prefix,
        );
        Ok(info)
    }

    pub async fn check_connection(
        &self,
    ) -> Result<async_nats::connection::State, async_nats::Error> {
        let conn_state = self.client.connection_state();
        if let async_nats::connection::State::Disconnected = conn_state {
            Err(Box::new(ErrClientDisconnected))
        } else {
            Ok(conn_state)
        }
    }

    pub async fn publish(
        &self,
        payload: PublishInfo,
    ) -> Result<(), async_nats::error::Error<async_nats::jetstream::context::PublishErrorKind>>
    {
        log::debug!(
            "{}Called Publish message: subj={}, msg_id={} data={}",
            self.service_log_prefix,
            payload.subject,
            payload.msg_id,
            String::from_utf8_lossy(&payload.data),
        );

        let now = Instant::now();
        let result = match payload.headers {
            Some(headers) => {
                self.js_context
                    .publish_with_headers(
                        payload.subject.clone(),
                        headers,
                        payload.data.clone().into(),
                    )
                    .await
            }
            None => {
                self.js_context
                    .publish(payload.subject.clone(), payload.data.clone().into())
                    .await
            }
        };

        let duration = now.elapsed();
        if let Err(err) = result {
            if let Some(ref on_failed) = self.on_msg_failed_event {
                on_failed(&payload.subject, &self.name, duration); // todo: add msg_id
            }
            return Err(err);
        }

        if let Some(ref on_published) = self.on_msg_published_event {
            on_published(&payload.subject, &self.name, duration);
        }
        Ok(())
    }

    pub async fn add_js_service(
        &mut self,
        params: JsServiceBuilder,
    ) -> Result<(), async_nats::Error> {
        let new_service = JsStreamService::new(
            self.js_context.to_owned(),
            &params.name,
            &params.description,
            &params.version,
            &params.service_subject,
        )
        .await?;

        let mut current_services = self.js_services.to_owned().unwrap_or_default();
        current_services.push(new_service);
        self.js_services = Some(current_services);

        Ok(())
    }

    pub async fn get_js_service(&self, js_service_name: String) -> Option<&JsStreamService> {
        if let Some(services) = &self.js_services {
            return services
                .iter()
                .find(|s| s.get_service_info().name == js_service_name);
        }
        None
    }

    pub async fn close(&self) -> Result<(), async_nats::Error> {
        self.client.drain().await?;
        Ok(())
    }
}

// Client Options:
pub fn with_event_listeners(listeners: Vec<EventListener>) -> EventListener {
    Arc::new(Box::new(move |c: &mut JsClient| {
        for listener in &listeners {
            listener(c);
        }
    }))
}

// Event Listener Options:
pub fn on_msg_published_event<F>(f: F) -> EventListener
where
    F: Fn(&str, &str, Duration) + Send + Sync + Clone + 'static,
{
    Arc::new(Box::new(move |c: &mut JsClient| {
        c.on_msg_published_event = Some(Arc::new(Box::pin(f.clone())));
    }))
}

pub fn on_msg_failed_event<F>(f: F) -> EventListener
where
    F: Fn(&str, &str, Duration) + Send + Sync + Clone + 'static,
{
    Arc::new(Box::new(move |c: &mut JsClient| {
        c.on_msg_failed_event = Some(Arc::new(Box::pin(f.clone())));
    }))
}

pub fn get_event_listeners() -> Vec<EventListener> {
    // TODO: Use duration in handlers..
    let published_msg_handler = move |msg: &str, client_name: &str, _duration: Duration| {
        log::info!("Successfully published message for {client_name}. Msg: {msg:?}",);
    };
    let failure_handler = |err: &str, client_name: &str, _duration: Duration| {
        log::info!("Failed to publish for {client_name}. Err: {err:?}");
    };

    let event_listeners = vec![
        on_msg_published_event(published_msg_handler), // Shouldn't this be the 'NATS_LISTEN_PORT'?
        on_msg_failed_event(failure_handler),
    ];

    event_listeners
}
