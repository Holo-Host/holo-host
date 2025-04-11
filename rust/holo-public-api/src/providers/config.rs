use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub mongo_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub enable_swagger: bool,
    pub enable_scheduler: bool,
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
