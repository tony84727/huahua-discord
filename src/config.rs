use std::io::ErrorKind;

use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{self, AsyncReadExt},
};

fn localhost() -> String {
    "localhost".to_string()
}

#[derive(Serialize, Deserialize)]
pub struct Database {
    pub user: String,
    pub password: String,
    #[serde(default = "localhost")]
    pub host: String,
}

impl Database {
    pub fn connection_string(&self) -> String {
        format!("mongodb://{}:{}@{}", self.user, self.password, self.host)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Bot {
    pub token: String,
    pub guild_id: u64,
    pub application_id: u64,
    pub database: Database,
}

#[derive(Debug)]
pub enum ConfigLoadError {
    NoFound,
    Format(toml::de::Error),
    IO(io::Error),
}

impl Bot {
    pub async fn load() -> Result<Self, ConfigLoadError> {
        let mut config = File::open("./bot.toml").await.map_err(|err| match err {
            err if err.kind() == ErrorKind::NotFound => ConfigLoadError::NoFound,
            err => ConfigLoadError::IO(err),
        })?;
        let mut buffer = vec![];
        config
            .read_to_end(&mut buffer)
            .await
            .map_err(ConfigLoadError::IO)?;
        toml::from_slice(&buffer).map_err(ConfigLoadError::Format)
    }
}
