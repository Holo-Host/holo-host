use anyhow::Result;
use async_nats::jetstream::Context;
use async_nats::{jetstream::consumer::PullConsumer, Message};
use std::sync::Arc;
use tokio::{fs::File, io::AsyncWriteExt};

pub async fn receive_and_write_file(
    msg: Arc<Message>,
    output_dir: &str,
    file_name: &str,
) -> Result<()> {
    let output_path = format!("{}/{}", output_dir, file_name);
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)
        .await?;

    let payload_buf = msg.payload.to_vec();
    let payload = serde_json::from_slice::<String>(&payload_buf)?;
    if payload.to_string().contains("EOF") {
        log::info!("File transfer complete.");
        return Ok(());
    }

    file.write_all(&msg.payload).await?;
    file.flush().await?;

    Ok(())
}
