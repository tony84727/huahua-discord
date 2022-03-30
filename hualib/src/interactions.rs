use crate::fx::{Controller, Creator, Fx, MediaOrigin, PreviewingFx, Repository, Store};
use async_trait::async_trait;
use serenity::{
    builder::{CreateApplicationCommand, CreateEmbed},
    client::Context,
    model::{
        channel::{AttachmentType, Message},
        id::UserId,
        interactions::{
            application_command::{
                ApplicationCommandInteraction, ApplicationCommandInteractionDataOption,
                ApplicationCommandInteractionDataOptionValue, ApplicationCommandOptionType,
            },
            InteractionResponseType,
        },
    },
    utils::Colour,
};
use std::{borrow::Cow, time::Duration};

#[async_trait]
pub(crate) trait ChatCommand {
    fn create<'c>(
        &self,
        command: &'c mut CreateApplicationCommand,
    ) -> &'c mut CreateApplicationCommand;
    async fn exec(&self, ctx: &Context, interaction: &ApplicationCommandInteraction);
}

pub(crate) struct CreateFxCommand<'a, C, S, R>
where
    C: Creator,
    S: Store + 'static,
    R: Repository,
{
    controller: &'a Controller<C, S, R>,
}

fn check_message<R>(result: serenity::Result<R>) {
    if let Err(why) = result {
        log::error!("{:?}", why);
    }
}

#[async_trait]
impl<'a, C, S, R> ChatCommand for CreateFxCommand<'a, C, S, R>
where
    C: Creator,
    S: Store + 'static,
    R: Repository,
{
    fn create<'c>(
        &self,
        command: &'c mut CreateApplicationCommand,
    ) -> &'c mut CreateApplicationCommand {
        command
            .name("newfx")
            .description("創造音效指令")
            .create_option(|option| {
                option
                    .name("名稱")
                    .description("音效指令的名稱")
                    .kind(ApplicationCommandOptionType::String)
                    .required(true)
            })
            .create_option(|option| {
                option
                    .name("描述")
                    .description("音效指令的描述")
                    .kind(ApplicationCommandOptionType::String)
                    .required(true)
            })
            .create_option(|option| {
                option
                    .name("來源")
                    .description("填入影片的URL")
                    .kind(ApplicationCommandOptionType::String)
                    .required(true)
            })
            .create_option(|option| {
                option
                    .name("開始秒數")
                    .description("開始秒數，預設0秒開始")
                    .kind(ApplicationCommandOptionType::Number)
            })
            .create_option(|option| {
                option
                    .name("持續秒數")
                    .description("持續秒數，最大20秒，預設5秒")
                    .kind(ApplicationCommandOptionType::Number)
                    .max_int_value(20)
                    .min_int_value(1)
            })
    }
    async fn exec(&self, ctx: &Context, command: &ApplicationCommandInteraction) {
        check_message(
            command
                .create_interaction_response(ctx, |response| {
                    response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
                })
                .await,
        );
        let author = match &command.member {
            None => {
                return;
            }
            Some(member) => member.user.id,
        };
        if let Some(fx) = Self::option_fx(author, &command.data.options) {
            match self.controller.init_create_fx(fx).await {
                Ok(preview) => {
                    check_message(Self::post_preview(ctx, command, preview).await);
                }
                Err(why) => {
                    log::error!("{:?}", why);
                }
            }
        }
    }
}

impl<'a, C, S, R> CreateFxCommand<'a, C, S, R>
where
    C: Creator,
    S: Store,
    R: Repository,
{
    pub(crate) fn new(controller: &'a Controller<C, S, R>) -> Self {
        Self { controller }
    }
    async fn post_preview(
        ctx: &Context,
        interaction: &ApplicationCommandInteraction,
        preview: PreviewingFx,
    ) -> serenity::Result<Message> {
        let data = Cow::Borrowed(preview.media.as_slice());
        interaction
            .create_followup_message(ctx, |response| {
                let mut embed = CreateEmbed::default();
                embed
                    .colour(Colour::ORANGE)
                    .title(&preview.fx.name)
                    .description(preview.fx.description)
                    .field("連結", preview.fx.origin.url, false)
                    .field(
                        "開始秒數",
                        format!("{}秒", preview.fx.origin.start.as_secs()),
                        false,
                    )
                    .field(
                        "長度",
                        format!("{}秒", preview.fx.origin.length.as_secs()),
                        false,
                    );
                response.add_embed(embed).add_file(AttachmentType::Bytes {
                    data,
                    filename: format!("preview_{}.mp3", preview.fx.name),
                })
            })
            .await
    }
    fn option_fx(
        author: UserId,
        options: &[ApplicationCommandInteractionDataOption],
    ) -> Option<Fx> {
        let start = options
            .get(3)
            .and_then(|option| option.resolved.as_ref())
            .map(|value| match value {
                ApplicationCommandInteractionDataOptionValue::Integer(value) => *value as u64,
                _ => 0,
            })
            .unwrap_or(0_u64);
        let length = options
            .get(4)
            .and_then(|option| option.resolved.as_ref())
            .map(|value| match value {
                ApplicationCommandInteractionDataOptionValue::Integer(value) => {
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
            author,
            name: options
                .get(0)
                .and_then(|option| option.resolved.as_ref())
                .and_then(|value| match value {
                    ApplicationCommandInteractionDataOptionValue::String(value) => {
                        Some(value.clone())
                    }
                    _ => None,
                }),
            description: options
                .get(1)
                .and_then(|option| option.resolved.as_ref())
                .and_then(|value| match value {
                    ApplicationCommandInteractionDataOptionValue::String(value) => {
                        Some(value.clone())
                    }
                    _ => None,
                }),
            url: options
                .get(2)
                .and_then(|option| option.resolved.as_ref())
                .and_then(|value| match value {
                    ApplicationCommandInteractionDataOptionValue::String(value) => {
                        Some(value.clone())
                    }
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
    author: UserId,
}

impl FxArgument {
    fn to_fx(self) -> Option<Fx> {
        if self.name.is_some() && self.description.is_some() && self.url.is_some() {
            return Some(Fx {
                name: self.name.unwrap(),
                description: self.description.unwrap(),
                origin: MediaOrigin {
                    url: self.url.unwrap(),
                    start: Duration::from_secs(self.start),
                    length: Duration::from_secs(self.length),
                },
                author: self.author,
            });
        }
        None
    }
}
