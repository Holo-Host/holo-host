use nats_utils::{
    jetstream_client::{get_event_listeners, get_nats_url, with_event_listeners, JsClient},
    types::JsClientBuilder,
};
use std::{path::PathBuf, time::Duration};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_HPOS_INBOX";

pub async fn run(host_id: &str, host_creds_path: &Option<PathBuf>) -> anyhow::Result<JsClient> {
    let nats_url = get_nats_url();
    log::info!("nats_url : {nats_url}");
    log::info!("host_creds_path (currently omited) : {host_creds_path:?}");
    log::info!("host_id : {host_id}");

    let pubkey_lowercase: String = host_id.to_string().to_lowercase();

    let host_client = JsClient::new(JsClientBuilder {
        nats_url: nats_url.clone(),
        name: HOST_AGENT_CLIENT_NAME.to_string(),
        inbox_prefix: format!("{HOST_AGENT_INBOX_PREFIX}.{pubkey_lowercase}"),
        credentials: Default::default(),
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(29)),
        listeners: vec![with_event_listeners(get_event_listeners())],
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"))?;

    Ok(host_client)
}
