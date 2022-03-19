use std::io::ErrorKind;

use serde::{Deserialize, Serialize};
use tokio::{
    fs::File,
    io::{self, AsyncReadExt},
};

#[derive(Serialize, Deserialize)]
pub struct Bot {
    pub token: String,
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
