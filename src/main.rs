use std::env::VarError;

use async_trait::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::macros::group;
use serenity::framework::{
    standard::{macros::command, CommandResult},
    StandardFramework,
};
use serenity::model::channel::Message;
use tokio::fs::File;
use tokio::io::{self, AsyncReadExt};

mod music;

struct Handler;

#[async_trait]
impl EventHandler for Handler {}

#[group]
#[commands(ping)]
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

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&GENERAL_GROUP);
    let token = load_token().await;
    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("error while creating client");
    if let Err(why) = client.start().await {
        println!("an error occurred while running the client: {:?}", why);
    }
}
