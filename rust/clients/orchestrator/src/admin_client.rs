use nats_utils::{
    jetstream_client::{get_event_listeners, with_event_listeners, JsClient},
    types::{Credentials, JsClientBuilder},
};
use std::vec;
use std::{path::PathBuf, time::Duration};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "_ADMIN_INBOX.orchestrator";

pub async fn run(admin_creds_path: &Option<PathBuf>, nats_url: String) -> anyhow::Result<JsClient> {
    log::info!("nats_url : {nats_url}");
    log::info!("admin_creds_path : {admin_creds_path:?}",);

    let admin_creds = admin_creds_path
        .to_owned()
        .map(|p| vec![Credentials::Path(p)])
        .filter(|c| !c.is_empty());

    let admin_client = JsClient::new(JsClientBuilder {
        nats_url: nats_url.clone(),
        name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
        inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
        credentials: admin_creds,
        request_timeout: Some(Duration::from_secs(29)),
        ping_interval: Some(Duration::from_secs(10)),
        listeners: vec![with_event_listeners(get_event_listeners())],
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"))?;

    Ok(admin_client)
}
