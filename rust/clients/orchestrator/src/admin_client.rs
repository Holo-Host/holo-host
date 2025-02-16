use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use tempfile::tempdir;
use util_libs::{
    nats_js_client,
    nats_server::{
        JetStreamConfig, LeafNodeRemote, LeafNodeRemoteTlsConfig, LeafServer, LoggingOptions,
        LEAF_SERVER_CONFIG_PATH, LEAF_SERVER_DEFAULT_LISTEN_PORT,
    },
};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "ORCHESTRATOR._ADMIN_INBOX";

pub async fn run() -> anyhow::Result<nats_js_client::JsClient> {
    // ==================== Setup NATS ====================
    let nats_url = get_nats_url();
    let creds_path = Credentials::Path(PathBuf::from_str(&get_nats_creds_by_nsc(
        "HOLO", "ADMIN", "admin",
    ))?);
    let event_listeners = get_event_listeners();

    // ==================== Setup DB ====================
    let orchestrator_admin_client = tokio::select! {
        client = async {loop {
            let admin_client = JsClient::new(NewJsClientParams {
                nats_url,
                name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
                inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
                service_params: vec![],
                credentials: Some(creds_path),
                request_timeout:Some(Duration::from_secs(29)),
                ping_interval: Some(Duration::from_secs(10)),
                listeners: vec![nats_js_client::with_event_listeners(event_listeners)],
            })
            .await?
                .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"));

                match admin_client {
                    Ok(client) => break client,
                    Err(e) => {
                        let duration = tokio::time::Duration::from_millis(100);
                        log::warn!("{}, retrying in {duration:?}", e);
                        tokio::time::sleep(duration).await;
                    }
                }
            }} => client,
        _ = {
            log::debug!("will time out waiting for NATS after {nats_connect_timeout_secs:?}");
            tokio::time::sleep(tokio::time::Duration::from_secs(nats_connect_timeout_secs))
        } => {
            anyhow::bail!("timed out waiting for NATS on {nats_url}");
        }
    };

    Ok(nats_client)
}
