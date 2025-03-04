use serde::{Deserialize, Serialize};
use config::{Config, File, Environment};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
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

    if config.database_url.is_empty() {
        return Err(config::ConfigError::Message("database_url is not set".to_string()));
    }

    Ok(config)
}