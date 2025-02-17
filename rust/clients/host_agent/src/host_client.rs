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

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_host_inbox";

pub async fn run(
    host_pubkey: &str,
    host_creds_path: &Option<PathBuf>,
    nats_connect_timeout_secs: u64,
) -> anyhow::Result<nats_js_client::JsClient> {
    let event_listeners = get_event_listeners();    
    let nats_url = nats_js_client::get_nats_url();
    log::info!("nats_url : {}", nats_url);

    log::info!("host_creds_path : {:?}", host_creds_path);
    log::info!("host_pubkey : {}", host_pubkey);

    let orchestrator_admin_client = tokio::select! {
        client = async {loop {
            let admin_client = JsClient::new(NewJsClientParams {
                nats_url,
                name: HOST_AGENT_CLIENT_NAME.to_string(),
                inbox_prefix: format!("{}_{}", HOST_AGENT_INBOX_PREFIX, host_pubkey),
                service_params: Default::default(),
                credentials_path: host_creds_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                opts: vec![nats_js_client::with_event_listeners(event_listeners)],
                request_timeout:Some(Duration::from_secs(29)),
                ping_interval: Some(Duration::from_secs(10)),
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
