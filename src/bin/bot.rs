use mongodb::Client as MongodbClient;
use serenity::client::Client;
use serenity::framework::StandardFramework;
use serenity::model::gateway::GatewayIntents;
use songbird::SerenityInit;

use huahua_discord::bot::Handler;
use huahua_discord::config;
use huahua_discord::music::MUSIC_GROUP;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_module("hualib", log::LevelFilter::Debug)
        .init();
    let bot_config = config::Bot::load().await.expect("fail to load bot config");
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("!"))
        .group(&MUSIC_GROUP);
    let mongo_client = MongodbClient::with_uri_str(bot_config.database.connection_string())
        .await
        .expect("initializing mongodb client");

    let database = mongo_client.database("huahua");
    let mut client = Client::builder(bot_config.token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .event_handler(Handler::new(database))
        .application_id(bot_config.application_id)
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
