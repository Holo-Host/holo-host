use std::{path::PathBuf, time::Duration};
use util_libs::nats::{
    jetstream_client::{get_event_listeners, get_nats_url, with_event_listeners, JsClient},
    types::JsClientBuilder,
};

const HOST_AGENT_CLIENT_NAME: &str = "Host Agent";
const HOST_AGENT_INBOX_PREFIX: &str = "_WORKLOAD_INBOX";

pub async fn run(
    host_pubkey: &str,
    host_creds_path: &Option<PathBuf>,
    nats_connect_timeout_secs: u64,
) -> anyhow::Result<JsClient> {
    let nats_url = get_nats_url();
    log::info!("nats_url : {}", nats_url);
    log::info!("host_creds_path : {:?}", host_creds_path);
    log::info!("host_pubkey : {}", host_pubkey);

    let pubkey_lowercase: String = host_pubkey.to_string().to_lowercase();

    let creds = host_creds_path
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());

    // Spin up Nats Client and loaded in the Js Stream Service
    // Nats takes a moment to become responsive, so we try to connect in a loop for a few seconds.
    // TODO: how do we recover from a connection loss to Nats in case it crashes or something else?
    let host_client = tokio::select! {
        client = async {loop {
            let c = JsClient::new(JsClientBuilder {
                nats_url: nats_url.clone(),
                name: HOST_AGENT_CLIENT_NAME.to_string(),
                inbox_prefix: format!("{}.{}", pubkey_lowercase, HOST_AGENT_INBOX_PREFIX),
                credentials_path: creds.clone(),
                ping_interval: Some(Duration::from_secs(10)),
                request_timeout:Some(Duration::from_secs(29)),
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

    Ok(host_client)
}
