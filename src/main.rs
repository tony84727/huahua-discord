use std::env::VarError;

use async_trait::async_trait;
use serenity::client::{Client, EventHandler};
use serenity::framework::standard::macros::group;
use serenity::framework::StandardFramework;
use songbird::SerenityInit;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

mod music;
use music::{JOIN_COMMAND, PLAY_COMMAND, STOP_COMMAND, TBC_COMMAND};

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[group]
#[commands(join, play, stop, tbc)]
struct General;

async fn load_token_file() -> io::Result<String> {
    let mut f = File::open("TOKEN").await?;
    let mut buf = vec![];
    f.read_to_end(&mut buf).await?;
    Ok(String::from_utf8(buf).unwrap())
}

async fn load_token() -> String {
    match std::env::var("TOKEN") {
        Ok(token) => {
            return token;
        }
        Err(err) if err == VarError::NotPresent => {
            return load_token_file().await.unwrap();
        }
        Err(err) => {
            panic!("{}", err);
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_module("huahua_discord", log::LevelFilter::Info)
        .init();
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP);
    let token = load_token().await;
    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("error while creating client");
    tokio::spawn(async move {
        let _ = client
            .start()
            .await
            .map_err(|why| log::error!("client stopped: {:?}", why));
    });
    tokio::signal::ctrl_c().await.unwrap();
    println!("received ctrl-c, shutting down");
}
