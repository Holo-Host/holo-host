use anyhow::anyhow;
use nats_utils::{
    jetstream_client::{
        get_event_listeners, get_nats_creds_by_nsc, get_nats_url, with_event_listeners, JsClient,
    },
    types::{Credentials, JsClientBuilder},
};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::vec;

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "_ADMIN_INBOX.orchestrator";

pub async fn run(admin_creds_path: &Option<PathBuf>) -> anyhow::Result<JsClient> {
    let nats_url = get_nats_url();
    log::info!("nats_url : {}", nats_url);

    let creds_path = admin_creds_path
        .to_owned()
        .ok_or(PathBuf::from_str(&get_nats_creds_by_nsc(
            "HOLO", "ADMIN", "admin",
        )))
        .map(Credentials::Path)
        .map_err(|e| anyhow!("Failed to locate admin credential path. Err={:?}", e))?;

    log::info!("final admin creds path : {:?}", creds_path);

    let admin_client = JsClient::new(JsClientBuilder {
        nats_url: nats_url.clone(),
        name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
        inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
        credentials: Some(vec![creds_path.clone()]),
        request_timeout: Some(Duration::from_secs(29)),
        ping_interval: Some(Duration::from_secs(10)),
        listeners: vec![with_event_listeners(get_event_listeners())],
    })
    .await
    .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"))?;

    Ok(admin_client)
}
