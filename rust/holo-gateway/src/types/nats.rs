use async_nats::Client;
use nats_utils::types::NATS_URL_DEFAULT;
use tokio::sync::OnceCell;

pub const HTTP_GW_SUBJECT_NAME: &str = "HC_HTTP_GW";

static NATS_CLIENT: OnceCell<Client> = OnceCell::const_new();
pub async fn get_nats_client() -> &'static Client {
    NATS_CLIENT
        .get_or_init(|| async {
            crate::nats_client::run()
                .await
                .expect("Failed to initialize NATS client")
        })
        .await
}

pub fn get_nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| NATS_URL_DEFAULT.to_string())
}

pub fn get_holo_gw_admin_credential_path() -> String {
    std::env::var("HOLO_GW_ADMIN_CRED_PATH")
        .unwrap_or_else(|_| "/var/lib/holo-gw-agent/admin.creds".to_string())
}
