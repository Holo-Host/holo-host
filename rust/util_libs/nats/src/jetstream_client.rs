use super::{
    jetstream_service::JsStreamService,
    leaf_server::LEAF_SERVER_DEFAULT_LISTEN_PORT,
    types::{
        Credentials, ErrClientDisconnected, EventHandler, EventListener, JsClientBuilder,
        JsServiceBuilder, PublishInfo,
    },
};
use anyhow::Result;
use async_nats::{jetstream, ServerInfo};
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
    url: String,
    pub name: String,
    on_msg_published_event: Option<EventHandler>,
    on_msg_failed_event: Option<EventHandler>,
    pub js_services: Option<Vec<JsStreamService>>,
    pub js_context: jetstream::Context,
    service_log_prefix: String,
    client: async_nats::Client, // Built-in Nats Client which manages the cloned clients within jetstream contexts
}

impl JsClient {
    pub async fn new(p: JsClientBuilder) -> Result<Self, async_nats::Error> {
        let mut connect_options = async_nats::ConnectOptions::new()
            .name(&p.name)
            .ping_interval(p.ping_interval.unwrap_or(Duration::from_secs(120)))
            .request_timeout(Some(p.request_timeout.unwrap_or(Duration::from_secs(10))))
            .custom_inbox_prefix(&p.inbox_prefix);
        // .require_tls(true)

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

        let client = connect_options.connect(&p.nats_url).await?;
        let service_log_prefix = format!("NATS-CLIENT-LOG::{}::", p.name);
        log::info!(
            "{service_log_prefix}Connected to NATS server at {}",
            p.nats_url
        );

        let mut js_client = JsClient {
            url: p.nats_url,
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
            "{}Called Publish message: subj={}, msg_id={} data={:?}",
            self.service_log_prefix,
            payload.subject,
            payload.msg_id,
            payload.data
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

// Helpers:
// TODO: there's overlap with the NATS_LISTEN_PORT. refactor this to e.g. read NATS_LISTEN_HOST and NATS_LISTEN_PORT
pub fn get_nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| {
        let default = format!("127.0.0.1:{LEAF_SERVER_DEFAULT_LISTEN_PORT}"); // Shouldn't this be the 'NATS_LISTEN_PORT'?
        log::debug!("using default for NATS_URL: {default}");
        default
    })
}

fn get_nsc_root_path() -> String {
    std::env::var("NSC_PATH").unwrap_or_else(|_| "/.local/share/nats/nsc".to_string())
}

pub fn get_local_creds_path() -> String {
    std::env::var("LOCAL_CREDS_PATH")
        .unwrap_or_else(|_| format!("{}/local_creds", get_nsc_root_path()))
}

pub fn get_nats_creds_by_nsc(operator: &str, account: &str, user: &str) -> String {
    format!(
        "{}/keys/creds/{}/{}/{}.creds",
        get_nsc_root_path(),
        operator,
        account,
        user
    )
}

pub fn get_nats_jwt_by_nsc(operator: &str, account: &str, user: &str) -> String {
    format!(
        "{}/stores/{}/accounts/{}/users/{}.jwt",
        get_nsc_root_path(),
        operator,
        account,
        user
    )
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
