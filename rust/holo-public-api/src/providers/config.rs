use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    /// the current address of the server (http://localhost:3000)
    pub host: String,
    /// port to run the server on
    pub port: u16,
    /// connection string for mongodb
    pub mongo_url: String,
    /// connection string for redis
    pub redis_url: String,
    /// secret used to sign jwt tokens
    pub jwt_secret: String,
    /// enable internal documentation
    pub enable_documentation: bool,
    /// enable scheduler to run cron jobs
    pub enable_scheduler: bool,
    /// defaults to '/tmp'
    pub temp_storage_location: Option<String>,
    /// defaults to '.'
    pub blob_storage_location: Option<String>,
    /// defaults to 5 minutes (in seconds)
    pub access_token_expiry: Option<u64>,
}

pub fn load_config() -> Result<AppConfig, config::ConfigError> {
    dotenvy::dotenv().ok();

    let settings = Config::builder()
        .set_default("port", 3000)?
        .add_source(File::with_name(".env").required(false))
        .add_source(Environment::default())
        .build()
        .unwrap();

    let config: AppConfig = settings.try_deserialize()?;
    Ok(config)
}
