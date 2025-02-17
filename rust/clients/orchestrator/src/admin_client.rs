use anyhow::anyhow;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::vec;
use util_libs::nats_js_client::{
    self, get_event_listeners, get_nats_creds_by_nsc, get_nats_url, Credentials, JsClient,
    NewJsClientParams,
};

const ORCHESTRATOR_ADMIN_CLIENT_NAME: &str = "Orchestrator Admin Client";
const ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX: &str = "ORCHESTRATOR._ADMIN_INBOX";

pub async fn run(
    admin_creds_path: &Option<PathBuf>,
    nats_connect_timeout_secs: u64,
) -> anyhow::Result<JsClient> {
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

    let admin_client = tokio::select! {
        client = async {loop {
            let c = JsClient::new(NewJsClientParams {
                nats_url: nats_url.clone(),
                name: ORCHESTRATOR_ADMIN_CLIENT_NAME.to_string(),
                inbox_prefix: ORCHESTRATOR_ADMIN_CLIENT_INBOX_PREFIX.to_string(),
                service_params: vec![],
                credentials: Some(vec![creds_path.clone()]),
                request_timeout:Some(Duration::from_secs(29)),
                ping_interval: Some(Duration::from_secs(10)),
                listeners: vec![nats_js_client::with_event_listeners(get_event_listeners())],
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
