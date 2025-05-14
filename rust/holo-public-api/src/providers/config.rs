use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    /// REQUIRED
    /// connection string for mongodb
    pub mongo_url: String,
    /// connection string for redis
    pub redis_url: String,
    /// secret used to sign jwt tokens
    pub jwt_secret: String,

    /// OPTIONAL
    /// the current address of the server, defaults to http://localhost:3000
    pub host: Option<String>,
    /// port to run the server on, defaults to 3000
    pub port: Option<u16>,
    /// enable internal documentation, defaults to false
    pub enable_internal_docs: Option<bool>,
    /// enable scheduler to run cron jobs, defaults to false
    pub enable_scheduler: Option<bool>,
    /// defaults to '/tmp'
    pub temp_storage_location: Option<String>,
    /// defaults to '.'
    pub blob_storage_location: Option<String>,
    /// defaults to 5 minutes (in seconds)
    pub access_token_expiry: Option<u64>,
    /// defaults to 100
    pub rate_limit_max_requests: Option<i32>,
    /// defaults to 60 seconds
    pub rate_limit_window: Option<i32>,
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
