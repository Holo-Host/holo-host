use async_nats::ServerAddr;
use nats_utils::{
    jetstream_client::{get_event_listeners, with_event_listeners, JsClient},
    types::{Credentials, JsClientBuilder},
};
use std::vec;
use std::{path::PathBuf, time::Duration};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "_ADMIN_INBOX.orchestrator";

pub async fn run(
    admin_creds_path: &Option<PathBuf>,
    nats_url: &ServerAddr,
    maybe_nats_user: Option<String>,
    maybe_nats_password_file: Option<PathBuf>,
    nats_skip_tls_verification_danger: bool,
) -> anyhow::Result<JsClient> {
    log::info!("nats_url : {nats_url:?}");

    let credentials = admin_creds_path
        .to_owned()
        .map(|creds| vec![Credentials::Path(creds)]);

    let admin_client = JsClient::new(JsClientBuilder {
        nats_url: nats_url.into(),
        name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
        inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
        credentials,
        request_timeout: Some(Duration::from_secs(29)),
        ping_interval: Some(Duration::from_secs(10)),
        listeners: vec![with_event_listeners(get_event_listeners())],
        maybe_nats_user,
        maybe_nats_password_file,
        nats_skip_tls_verification_danger,
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url:?}: {e}"))?;

    Ok(admin_client)
}
