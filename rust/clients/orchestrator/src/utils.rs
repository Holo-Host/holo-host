use super::auth::controller::ORCHESTRATOR_AUTH_CLIENT_NAME;
use anyhow::Result;
use std::io::Read;
use tokio::time::Duration;
use util_libs::nats_js_client::{self, EventListener, JsClient, SendRequest};

const CHUNK_SIZE: usize = 1024; // 1 KB chunk size

pub fn get_hpos_users_pubkey_path() -> String {
    std::env::var("RESOLVER_FILE_PATH").unwrap_or_else(|_| "./resolver.conf".to_string())
}

pub fn get_resolver_path() -> String {
    std::env::var("RESOLVER_FILE_PATH").unwrap_or_else(|_| "./resolver.conf".to_string())
}

pub async fn chunk_file_and_publish(
    auth_client: &JsClient,
    subject: &str,
    host_id: &str,
) -> Result<()> {
    let file_path = format!("{}/{}.jwt", get_hpos_users_pubkey_path(), host_id);
    let mut file = std::fs::File::open(file_path)?;
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut chunk_id = 0;

    while let Ok(bytes_read) = file.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }
        let chunk_data = &buffer[..bytes_read];

        let send_user_jwt_publish_options = SendRequest {
            subject: subject.to_string(),
            msg_id: format!("hpos_init_msg_id_{}", rand::random::<u8>()),
            data: chunk_data.into(),
        };
        auth_client.publish(&send_user_jwt_publish_options).await;
        chunk_id += 1;
    }

    // Send an EOF marker
    let send_user_jwt_publish_options = SendRequest {
        subject: subject.to_string(),
        msg_id: format!("hpos_init_msg_id_{}", rand::random::<u8>()),
        data: "EOF".into(),
    };
    auth_client.publish(&send_user_jwt_publish_options);

    Ok(())
}
