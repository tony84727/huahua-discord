use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serenity::{
    client::{Context, EventHandler},
    model::{id::GuildId, interactions::Interaction, prelude::Ready},
};

use crate::{
    fx::{
        self, CachedCreator, Creator, LocalStore, MongoDBRepository, Repository, YoutubeDLCreator,
    },
    interactions::{data::InteractionDataRegistry, fx::CreateFxCommand, ButtonHandler},
};
pub struct Handler<C, R>
where
    C: Creator,
    R: Repository,
{
    controller: fx::Controller<C, R>,
    database: mongodb::Database,
    interaction_data_registry: InteractionDataRegistry,
}

#[async_trait]
impl<C, R> EventHandler for Handler<C, R>
where
    C: Creator,
    R: Repository,
{
    async fn ready(&self, ctx: Context, _ready: Ready) {
        let guilds = self.get_existing_guild_ids().await.unwrap();
        let command = CreateFxCommand::new(&self.controller, &self.interaction_data_registry);
        for guild in guilds {
            match guild
                .create_application_command(&ctx, |commands| command.create(commands))
                .await
            {
                Ok(_) => {
                    log::info!("created application command for guild {:?}", guild);
                }
                Err(why) => {
                    log::error!(
                        "fail to create application command for guild {:?}, err: {:?}",
                        guild,
                        why
                    );
                }
            }
        }
        log::info!("application commands initialized");
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command_interaction) => {
                // match command
                log::info!(
                    "received application command: {}",
                    command_interaction.data.name
                );
                match command_interaction.data.name.as_str() {
                    "fx" => {
                        log::info!("newfx invoked");
                        let command =
                            CreateFxCommand::new(&self.controller, &self.interaction_data_registry);
                        command.exec(&ctx, &command_interaction).await;
                    }
                    _ => (),
                }
            }
            Interaction::MessageComponent(component_interaction) => {
                let handler = ButtonHandler::new(&self.controller, self.database.clone());
                handler.handle(&ctx, &component_interaction).await;
            }
            _ => (),
        }
    }
}

impl<C, R> Handler<C, R>
where
    C: Creator,
    R: Repository,
{
    async fn get_existing_guild_ids(&self) -> mongodb::error::Result<Vec<GuildId>> {
        let mut guilds = self
            .database
            .collection::<GuildRecord>("guilds")
            .find(None, None)
            .await?;
        let mut guild_ids = vec![];
        while let Some(next) = guilds.next().await {
            match next {
                Err(why) => {
                    return Err(why);
                }
                Ok(GuildRecord { id }) => {
                    let id = match id.parse() {
                        Ok(id) => id,
                        Err(why) => {
                            log::error!("{} is not a valid guild id, err: {:?}", id, why);
                            continue;
                        }
                    };
                    guild_ids.push(GuildId(id));
                }
            }
        }
        Ok(guild_ids)
    }
}

impl Handler<CachedCreator<YoutubeDLCreator, LocalStore>, MongoDBRepository> {
    pub fn new(database: mongodb::Database) -> Self {
        let store = fx::LocalStore::new("fx");
        let repository = fx::MongoDBRepository::new(database.clone());
        let controller = fx::Controller::new(
            fx::CachedCreator::new(fx::YoutubeDLCreator, store),
            repository,
        );
        let interaction_data_registry = InteractionDataRegistry::new(database.clone());
        Self {
            controller,
            database,
            interaction_data_registry,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct GuildRecord {
    id: String,
}
