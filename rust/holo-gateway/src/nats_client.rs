use crate::types;
use types::nats as nats_types;

use async_nats::Client;
use std::{path::PathBuf, str::FromStr, time::Duration};

const HOLO_GATEWAY_ADMIN_CLIENT_NAME: &str = "Holo Gateway Admin Client";
const HOLO_GATEWAY_ADMIN_CLIENT_INBOX_PREFIX: &str = "_ADMIN_INBOX.holo_gateway";

pub async fn run() -> anyhow::Result<Client> {
    let nats_url = nats_types::get_nats_url();
    log::info!("nats_url : {nats_url:?}");

    let credentials = PathBuf::from_str(&nats_types::get_holo_gw_admin_credential_path())?;
    log::info!("credentials : {credentials:?}");

    let admin_client = async_nats::ConnectOptions::new()
        .name(HOLO_GATEWAY_ADMIN_CLIENT_NAME.to_string())
        .custom_inbox_prefix(HOLO_GATEWAY_ADMIN_CLIENT_INBOX_PREFIX.to_string())
        .credentials_file(&credentials)
        .await?
        .ping_interval(Duration::from_secs(10))
        .request_timeout(Some(Duration::from_secs(29)))
        .connect(nats_url)
        .await?;

    Ok(admin_client)
}
