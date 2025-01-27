use anyhow::Result;
use async_nats::jetstream::Context;
use util_libs::nats_js_client::ServiceError;
use std::io::Write;
use std::path::PathBuf;

pub fn handle_internal_err(err_msg: &str) -> ServiceError {
    log::error!("{}", err_msg);
    ServiceError::Internal(err_msg.to_string())
}

pub fn get_file_path_buf(
    file_name: &str,
) -> PathBuf {
    let root_path = std::env::current_dir().expect("Failed to locate root directory.");
    root_path.join(file_name)
}

pub async fn receive_and_write_file(
    data: Vec<u8>,
    output_dir: &str,
    file_name: &str,
) -> Result<String> {
    let output_path = format!("{}/{}", output_dir, file_name);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&output_path)?;

    file.write_all(&data)?;
    file.flush()?;
    Ok(output_path)
}

pub async fn publish_chunks(js: &Context, subject: &str, file_name: &str, data: Vec<u8>) -> Result<()> {
    // let data: Vec<u8> = std::fs::read(file_path)?;
    js.publish(format!("{}.{} ", subject, file_name), data.into()).await?;
    Ok(())
}

// Placeholder functions for the missing implementations
pub fn get_account_signing_key() -> String {
    // Implementation here
    String::new()
}

pub fn generate_user_jwt(_user_public_key: &str, _account_signing_key: &str) -> Option<String> {
    // Implementation here

    // // Output jwt with nsc
    // let user_jwt_path = Command::new("nsc")
    //     .arg("...")
    //     // .arg(format!("> {}", output_dir))
    //     .output()
    //     .expect("Failed to output user jwt to file")
    //     .stdout;

        Some(String::new())
}

// pub async fn chunk_file_and_publish(_js: &Context, _subject: &str, _file_path: &str) -> Result<()> {
    // let mut file = std::fs::File::open(file_path)?;
    // let mut buffer = vec![0; CHUNK_SIZE];
    // let mut chunk_id = 0;

    // while let Ok(bytes_read) = file.read(mut buffer) {
    //     if bytes_read == 0 {
    //         break;
    //     }
    //     let chunk_data = &buffer[..bytes_read];
    //     js.publish(subject.to_string(), chunk_data.into()).await.unwrap();
    //     chunk_id += 1;
    // }

    // // Send an EOF marker
    // js.publish(subject.to_string(), "EOF".into()).await.unwrap();

//     Ok(())
// }
