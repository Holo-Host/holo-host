use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{config::{Credentials, SharedCredentialsProvider}, Client};

use super::config::AppConfig;

pub async fn setup_object_storage(
    config: AppConfig,
) -> Result<Client, anyhow::Error> {

    let credentials = SharedCredentialsProvider::new(
        Credentials::new(
            config.object_storage_access_key,
            config.object_storage_secret_key,
            None,
            None,
            "digitalocean"
        )
    );

    let config = aws_config::load_defaults(BehaviorVersion::latest())
        .await.into_builder()
        .region(Region::new("eu-central-1"))
        .endpoint_url(format!("https://{}", config.object_storage_endpoint))
        .credentials_provider(credentials)
        .build();
    
    let client = Client::new(&config);

    Ok(client)
}