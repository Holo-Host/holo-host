use async_nats::ServerAddr;
use nats_utils::{
    jetstream_client::{get_event_listeners, with_event_listeners, JsClient},
    types::{Credentials, JsClientBuilder, NatsRemoteArgs},
};
use std::{path::PathBuf, time::Duration};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_HPOS_INBOX";

pub async fn run(
    device_id: &str,
    host_creds_path: &Option<PathBuf>,
    nats_url: &ServerAddr,
) -> anyhow::Result<JsClient> {
    log::info!("nats_url : {nats_url:?}");
    log::info!("host_creds_path (currently omited) : {host_creds_path:?}");
    log::info!("device_id : {device_id}");

    // was pubkey_lowercase <- f/u on this in the orhcestrator auth service...
    let device_id_lowercase: String = device_id.to_string().to_lowercase();

    let host_creds = host_creds_path
        .to_owned()
        .map(|p| vec![Credentials::Path(p)])
        .filter(|c| !c.is_empty());

    let host_client = JsClient::new(JsClientBuilder {
        nats_remote_args: NatsRemoteArgs {
            nats_url: nats_url.into(),
            ..Default::default()
        },

        name: HOST_AGENT_CLIENT_NAME.to_string(),
        inbox_prefix: format!("{HOST_AGENT_INBOX_PREFIX}.{device_id_lowercase}"),
        credentials: host_creds,
        ping_interval: Some(Duration::from_secs(10)),
        request_timeout: Some(Duration::from_secs(29)),
        listeners: vec![with_event_listeners(get_event_listeners())],
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url:?}: {e}"))?;

    Ok(host_client)
}
