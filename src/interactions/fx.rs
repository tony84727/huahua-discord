use crate::{
    audio::{mp3_to_songbird_input, try_join_authors_channel, try_play_source},
    discord::InteractionWrapper,
    fx::{
        Controller, Creator, DiscordOrigin, Fx, FxIdentity, GetFxError, MediaOrigin, PreviewingFx,
        Repository, RepositoryGetError,
    },
};
use rand::{distributions::Uniform, prelude::Distribution};
use serenity::{
    all::{CommandInteraction, CommandOptionType},
    builder::{CreateCommand, CreateCommandOption},
    client::Context,
};
use std::{borrow::Cow, io::Cursor, time::Duration};

use super::data::{InteractionData, InteractionDataRegistry};

impl<B> From<FnMut(B) -> B> for B
where
    B: Default,
{
    fn from(value: FnOnce<B>) -> Self {
        value(B::default())
    }
}

pub(crate) struct CreateFxCommand<'a, C, R>
where
    C: Creator,
    R: Repository,
{
    controller: &'a Controller<C, R>,
    data: &'a InteractionDataRegistry,
}

fn check_message<R>(result: serenity::Result<R>) {
    if let Err(why) = result {
        log::error!("{:?}", why);
    }
}

impl<'a, C, R> CreateFxCommand<'a, C, R>
where
    C: Creator,
    R: Repository,
{
    pub fn create<'c>(&self) -> CreateCommand {
        CreateCommand::new("fx")
            .description("音效指令")
            .add_option(
                CreateCommandOption::new(
                    serenity::all::CommandOptionType::SubCommand,
                    "create",
                    "創立音效指令",
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "名稱", "音效指令的名稱")
                        .required(true),
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "描述", "音效指令的描述")
                        .required(true),
                )
                .add_sub_option(
                    CreateCommandOption::new(CommandOptionType::String, "來源", "填入影片的URL")
                        .required(true),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::Number,
                        "開始秒數",
                        "開始秒數，預設0秒開始",
                    )
                    .min_int_value(0),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::Number,
                        "持續秒數",
                        "持續秒數，最大20秒，預設5秒",
                    )
                    .max_int_value(20)
                    .min_int_value(1),
                ),
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "play", "播放音效指令")
                    .add_sub_option(CreateCommandOption::new(
                        CommandOptionType::String,
                        "名稱",
                        "音效指令的名稱",
                    )),
            )
    }
    pub async fn exec(&self, ctx: &Context, command: &CommandInteraction) {
        check_message(command.defer(ctx).await);
        let discord_origin: DiscordOrigin = command.clone().into();
        let subcommand = match command
            .data
            .options
            .get(0)
            .map(|option| option.name.as_str())
        {
            Some(name) => name,
            None => {
                return;
            }
        };
        match subcommand {
            "create" => {
                if let Some(fx) = Self::option_fx(
                    discord_origin,
                    &command.data.options.get(0).unwrap().options,
                ) {
                    check_message(Self::post_processing(ctx, command).await);
                    match self.controller.init_create_fx(fx).await {
                        Ok(preview) => match self.post_preview(ctx, command, preview).await {
                            Ok(_) => (),
                            Err(why) => {
                                log::error!("{:?}", why);
                            }
                        },
                        Err(why) => {
                            log::error!("{:?}", why);
                        }
                    }
                } else {
                    check_message(Self::post_invalid(ctx, command).await);
                }
            }
            "play" => {
                if let Some(name) = command
                    .data
                    .options
                    .get(0)
                    .unwrap()
                    .options
                    .get(0)
                    .and_then(|option| option.resolved.as_ref())
                    .and_then(|resolved| match resolved {
                        CommandDataOptionValue::String(name) => Some(name),
                        _ => None,
                    })
                {
                    let guild_id = command.guild_id.unwrap();
                    let identity = FxIdentity(guild_id, name.clone());
                    let fx_media = match self.controller.get(&identity).await {
                        Ok(fx) => fx,
                        Err(GetFxError::Repository(RepositoryGetError::NotFound)) => {
                            log::debug!("{:?} fx not found", &identity);
                            if let Err(why) = command
                                .create_followup_message(ctx, |response| {
                                    response.content("本毛找不到此指令")
                                })
                                .await
                            {
                                log::error!("{:?}", why);
                            }
                            return;
                        }
                        Err(why) => {
                            log::error!("{:?}", why);
                            return;
                        }
                    };
                    try_join_authors_channel(ctx, InteractionWrapper(ctx, command)).await;
                    if let Err(err) = try_play_source(
                        ctx,
                        guild_id,
                        mp3_to_songbird_input(Cursor::new(fx_media.1)),
                    )
                    .await
                    {
                        log::error!("{:?}", err);
                    }
                }
            }
            x => {
                log::error!("receving unsupported subcommand: `fx {}`", x)
            }
        }
    }
}

#[derive(Debug)]
enum CreateFxError {
    Serenity(serenity::Error),
    Data(mongodb::error::Error),
}

struct RandomMessage<'m>(&'m [&'static str]);

impl<'m> RandomMessage<'m> {
    fn new(messages: &'m [&'static str]) -> Self {
        Self(messages)
    }
    fn next(&self) -> &str {
        let index = self.random_index();
        self.0.get(index).unwrap()
    }

    fn random_index(&self) -> usize {
        let distribution = Uniform::from(0..self.0.len());
        distribution.sample(&mut rand::thread_rng())
    }
}

impl<'a, C, R> CreateFxCommand<'a, C, R>
where
    C: Creator,
    R: Repository,
{
    pub(crate) fn new(controller: &'a Controller<C, R>, data: &'a InteractionDataRegistry) -> Self {
        Self { controller, data }
    }
    async fn post_processing(
        ctx: &Context,
        interaction: &ApplicationCommandInteraction,
    ) -> serenity::Result<Message> {
        let random_message = RandomMessage::new(&[
            "喵! 本毛正在處理你的要求，雞肉條在特價噎，你應該知道本毛在說什麼？",
            "喵! 本毛正在處理你的要求",
            "喵! 本毛喜歡雞肉條跟罐罐。還有...本毛正在處理你的要求",
        ]);
        interaction
            .create_followup_message(ctx, |message| message.content(random_message.next()))
            .await
    }
    async fn post_preview(
        &self,
        ctx: &Context,
        interaction: &ApplicationCommandInteraction,
        preview: PreviewingFx,
    ) -> Result<Message, CreateFxError> {
        let data = Cow::Borrowed(preview.media.as_slice());
        let create_data_result = self
            .data
            .create(InteractionData::CreatingFx(preview.fx.clone()))
            .await
            .map_err(CreateFxError::Data)?;
        let id = create_data_result.inserted_id.as_object_id().unwrap();
        interaction
            .create_followup_message(ctx, |response| {
                let mut embed = CreateEmbed::default();
                embed
                    .colour(Colour::ORANGE)
                    .title(&preview.fx.name)
                    .description(preview.fx.description)
                    .field("連結", preview.fx.media.url, false)
                    .field(
                        "開始秒數",
                        format!("{}秒", preview.fx.media.start.as_secs()),
                        false,
                    )
                    .field(
                        "長度",
                        format!("{}秒", preview.fx.media.length.as_secs()),
                        false,
                    );
                response
                    .add_embed(embed)
                    .add_file(AttachmentType::Bytes {
                        data,
                        filename: format!("preview_{}.mp3", preview.fx.name),
                    })
                    .components(|component| {
                        component.create_action_row(|row| {
                            row.create_button(|button| {
                                button
                                    .style(ButtonStyle::Primary)
                                    .label("新增")
                                    .custom_id(format!("{}:create", id.to_hex()))
                            })
                            .create_button(|button| {
                                button
                                    .style(ButtonStyle::Secondary)
                                    .label("取消")
                                    .custom_id(format!("{}:cancel", id.to_hex()))
                            })
                        })
                    })
            })
            .await
            .map_err(CreateFxError::Serenity)
    }

    async fn post_invalid(
        ctx: &Context,
        interaction: &ApplicationCommandInteraction,
    ) -> serenity::Result<Message> {
        interaction
            .create_followup_message(ctx, |response| {
                response.content("本毛Don't know WTF are you talking about. 喵!")
            })
            .await
    }
    fn option_fx(discord: DiscordOrigin, options: &[CommandDataOption]) -> Option<Fx> {
        let start = options
            .get(3)
            .and_then(|option| option.resolved.as_ref())
            .map(|value| match value {
                CommandDataOptionValue::Integer(value) => *value as u64,
                _ => 0,
            })
            .unwrap_or(0_u64);
        let length = options
            .get(4)
            .and_then(|option| option.resolved.as_ref())
            .map(|value| match value {
                CommandDataOptionValue::Integer(value) => {
                    let value = *value;
                    if value == 0 {
                        5
                    } else {
                        value as u64
                    }
                }
                _ => 5,
            })
            .unwrap_or(5_u64);
        FxArgument {
            discord,
            name: options
                .get(0)
                .and_then(|option| option.resolved.as_ref())
                .and_then(|value| match value {
                    CommandDataOptionValue::String(value) => Some(value.clone()),
                    _ => None,
                }),
            description: options
                .get(1)
                .and_then(|option| option.resolved.as_ref())
                .and_then(|value| match value {
                    CommandDataOptionValue::String(value) => Some(value.clone()),
                    _ => None,
                }),
            url: options
                .get(2)
                .and_then(|option| option.resolved.as_ref())
                .and_then(|value| match value {
                    CommandDataOptionValue::String(value) => Some(value.clone()),
                    _ => None,
                }),
            start,
            length,
        }
        .to_fx()
    }
}

struct FxArgument {
    name: Option<String>,
    description: Option<String>,
    url: Option<String>,
    start: u64,
    length: u64,
    discord: DiscordOrigin,
}

impl FxArgument {
    fn to_fx(self) -> Option<Fx> {
        if self.name.is_some() && self.description.is_some() && self.url.is_some() {
            return Some(Fx {
                name: self.name.unwrap(),
                description: self.description.unwrap(),
                media: MediaOrigin {
                    url: self.url.unwrap(),
                    start: Duration::from_secs(self.start),
                    length: Duration::from_secs(self.length),
                },
                discord: self.discord,
            });
        }
        None
    }
}
