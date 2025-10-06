use std::env;

use serde::Deserialize;
use tracing::info;

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    pub env: String, // file / server
    pub host: String,
    pub port: u16,
    pub prefix: Option<String>,
}

pub fn get_config() -> Config {
    let env_var = env::var("env").unwrap_or("file".to_string());
    if env_var == "file" {
        info!("using .env file as environtment variable");
        let _ = dotenvy::dotenv();
    } else {
        info!("using server environtment as environtment variable");
    }
    envy::from_env::<Config>().unwrap()
}
