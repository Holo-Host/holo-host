use anyhow::Result;
use async_nats::jetstream::Context;
use std::process::Command;

pub async fn chunk_file_and_publish(_js: &Context, _subject: &str, _file_path: &str) -> Result<()> {
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

    Ok(())
}

pub fn generate_creds_file() -> String {
    let user_creds_path = "/path/to/host/user.creds".to_string();
    Command::new("nsc")
        .arg(format!("... > {}", user_creds_path))
        .output()
        .expect("Failed to add user with provided keys")
        .stdout;

    "placeholder_user.creds".to_string()
}
