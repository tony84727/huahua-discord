use std::{ffi::OsStr, fmt::Debug};

use serenity::{
    cache::FromStrAndCache,
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::{Channel, ChannelType, Message},
        id::{ChannelId, GuildId},
    },
};
use songbird::input::Input;
async fn try_join_channel(ctx: &Context, msg: &Message, channel_id: Option<ChannelId>) {
    match channel_id {
        Some(channel_id) => {
            join_channel(ctx, msg.guild_id.unwrap(), channel_id)
                .await
                .unwrap();
        }
        None => match find_voice_channel_of_user(ctx, msg).await {
            Some((guild_id, channel_id)) => match join_channel(ctx, guild_id, channel_id).await {
                Ok(()) => (),
                Err(err) => {
                    log::error!("fail to join voice channel, {:?}", err)
                }
            },
            None => {
                check_msg(msg.reply(ctx, format!("您沒有在任何語音頻道")).await);
            }
        },
    };
}

async fn try_parse_voice_channel_id(ctx: &Context, id: &str) -> Option<ChannelId> {
    let channel_id = match ChannelId::from_str(ctx, &id).await {
        Ok(id) => id,
        Err(_) => {
            return None;
        }
    };
    let channel = match channel_id.to_channel(ctx).await {
        Ok(channel) => channel,
        Err(_) => {
            return None;
        }
    };
    match channel {
        Channel::Guild(channel) if channel.kind == ChannelType::Voice => Some(channel_id),
        _ => {
            return None;
        }
    }
}

#[command]
pub async fn join(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let channel_id = args.single::<String>().ok();
    let channel_id = match channel_id {
        Some(id) => try_parse_voice_channel_id(ctx, &id).await,
        None => None,
    };
    try_join_channel(ctx, msg, channel_id).await;
    Ok(())
}

#[derive(Debug, PartialEq)]
enum PlayError {
    NotInChannel,
    CannotPlay,
}
async fn try_play_ytdl(
    ctx: &Context,
    msg: &Message,
    url: &str,
    guild_id: GuildId,
) -> Result<(), PlayError> {
    let source = match songbird::ytdl(&url).await {
        Ok(source) => source,
        Err(why) => {
            log::error!("cannot play youtube, url: {:?}", why);
            check_msg(msg.reply(&ctx.http, "無法播放QAQ").await);
            return Ok(());
        }
    };
    try_play_source(ctx, guild_id, source).await
}

async fn try_play_file<P: AsRef<OsStr> + Debug>(
    ctx: &Context,
    guild_id: GuildId,
    path: P,
) -> Result<(), PlayError> {
    let source = match songbird::ffmpeg(&path).await {
        Ok(input) => input,
        Err(err) => {
            log::error!("cannot play {:?} sound effect, {:?}", path, err);
            return Err(PlayError::CannotPlay);
        }
    };
    try_play_source(ctx, guild_id, source).await
}

async fn try_play_source(ctx: &Context, guild_id: GuildId, source: Input) -> Result<(), PlayError> {
    let manager = songbird::get(ctx).await.unwrap();
    match manager.get(guild_id) {
        Some(handler_lock) => {
            let mut handler = handler_lock.lock().await;
            handler.play_only_source(source);
            Ok(())
        }
        None => Err(PlayError::NotInChannel),
    }
}

#[command]
pub async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(msg.reply(&ctx.http, "用法!play <URL>").await);
            return Ok(());
        }
    };
    let guild = msg.guild(&ctx.cache).await.unwrap();
    let guild_id = guild.id;
    let play_result = try_play_ytdl(ctx, msg, &url, guild_id).await;
    match play_result {
        Err(err) if err == PlayError::NotInChannel => {
            try_join_channel(ctx, msg, None).await;
            try_play_ytdl(ctx, msg, &url, guild_id).await.unwrap();
        }
        _ => (),
    }
    Ok(())
}

#[command]
pub async fn tbc(ctx: &Context, msg: &Message) -> CommandResult {
    match try_play_file(ctx, msg.guild_id.unwrap(), "./resources/tc.mp3").await {
        Err(err) if err == PlayError::NotInChannel => {
            try_join_channel(ctx, msg, None).await;
            try_play_file(ctx, msg.guild_id.unwrap(), "./resources/tc.mp3")
                .await
                .unwrap();
        }
        _ => (),
    };
    Ok(())
}

#[command]
pub async fn pwtf(ctx: &Context, msg: &Message) -> CommandResult {
    match try_play_file(ctx, msg.guild_id.unwrap(), "./resources/pwtf.mp3").await {
        Err(err) if err == PlayError::NotInChannel => {
            try_join_channel(ctx, msg, None).await;
            try_play_file(ctx, msg.guild_id.unwrap(), "./resources/pwtf.mp3")
                .await
                .unwrap();
        }
        _ => (),
    };
    Ok(())
}

#[command]
pub async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        handler.leave().await.unwrap();
    } else {
        check_msg(msg.reply(ctx, "本毛沒在唱").await)
    }
    Ok(())
}

fn check_msg(result: serenity::Result<Message>) {
    if let Err(err) = result {
        log::error!("error sending message: {:?}", err);
    }
}

async fn join_channel(
    ctx: &Context,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<(), songbird::error::JoinError> {
    let manager = songbird::get(ctx)
        .await
        .expect("songbird failed to initialize")
        .clone();
    let (_handler, result) = manager.join(guild_id, channel_id).await;
    log::info!("joining {}/{}", guild_id, channel_id);
    result
}

async fn find_voice_channel_of_user(ctx: &Context, msg: &Message) -> Option<(GuildId, ChannelId)> {
    let channel_id = match msg
        .guild(&ctx.cache)
        .await
        .unwrap()
        .voice_states
        .get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id)
    {
        Some(channel_id) => channel_id,
        None => {
            return None;
        }
    };
    msg.guild_id.map(|guild_id| (guild_id, channel_id))
}
