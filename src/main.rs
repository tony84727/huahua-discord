use async_trait::async_trait;
use serenity::client::{Client, EventHandler};
use serenity::framework::standard::macros::group;
use serenity::framework::StandardFramework;
use songbird::SerenityInit;

mod config;
mod music;
use music::{JOIN_COMMAND, PLAY_COMMAND, PWTF_COMMAND, STOP_COMMAND, TBC_COMMAND};

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[group]
#[commands(join, play, stop, tbc, pwtf)]
struct General;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_module("huahua_discord", log::LevelFilter::Info)
        .init();
    let bot_config = config::Bot::load().await.expect("fail to load bot config");
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP);
    let mut client = Client::builder(bot_config.token)
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
