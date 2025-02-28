use std::path::PathBuf;
use std::time::Duration;
use std::vec;
use util_libs::nats::{
    jetstream_client::{get_event_listeners, get_nats_url, with_event_listeners, JsClient},
    types::JsClientBuilder,
};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "ORCHESTRATOR._ADMIN_INBOX";

pub async fn run(
    admin_creds_path: &Option<PathBuf>,
    nats_connect_timeout_secs: u64,
) -> anyhow::Result<JsClient> {
    let nats_url = get_nats_url();
    log::info!("nats_url : {}", nats_url);

    let creds = admin_creds_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());

    let admin_client = tokio::select! {
        client = async {loop {
            let c = JsClient::new(JsClientBuilder {
                nats_url: nats_url.clone(),
                name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
                inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
                credentials_path: creds.clone(),
                request_timeout:Some(Duration::from_secs(29)),
                ping_interval: Some(Duration::from_secs(10)),
                listeners: vec![with_event_listeners(get_event_listeners())],
            })
            .await
                .map_err(|e| anyhow::anyhow!("connecting to NATS via {nats_url}: {e}"));

                match c {
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

    Ok(admin_client)
}
