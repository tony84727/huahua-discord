use crate::{
    audio::{
        ensure_join_voice, stop_for_guild, try_join_channel, try_parse_voice_channel_id,
        try_play_file, try_play_ytdl, PlayError,
    },
    discord::check_msg,
};
use serenity::{
    client::Context,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};

#[group]
#[commands(join, play, stop, tbc, pwtf)]
struct Music;

#[command]
async fn join(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let channel_id = args.single::<String>().ok();
    let channel_id = match channel_id {
        Some(id) => try_parse_voice_channel_id(ctx, &id).await,
        None => None,
    };
    try_join_channel(ctx, msg, channel_id).await;
    Ok(())
}

#[command]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(msg.reply(&ctx.http, "用法!play <URL>").await);
            return Ok(());
        }
    };
    let guild = msg.guild(&ctx.cache).unwrap();
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
async fn tbc(ctx: &Context, msg: &Message) -> CommandResult {
    ensure_join_voice(ctx, msg).await;
    match try_play_file(ctx, msg.guild_id.unwrap(), "./resources/tc.mp3").await {
        Err(err) if err == PlayError::NotInChannel => {
            try_play_file(ctx, msg.guild_id.unwrap(), "./resources/tc.mp3")
                .await
                .unwrap();
        }
        _ => (),
    };
    Ok(())
}

#[command]
async fn pwtf(ctx: &Context, msg: &Message) -> CommandResult {
    ensure_join_voice(ctx, msg).await;
    match try_play_file(ctx, msg.guild_id.unwrap(), "./resources/pwtf.mp3").await {
        Err(err) if err == PlayError::NotInChannel => {
            try_play_file(ctx, msg.guild_id.unwrap(), "./resources/pwtf.mp3")
                .await
                .unwrap();
        }
        _ => (),
    };
    Ok(())
}

#[command]
async fn stop(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = msg.guild_id.unwrap();
    stop_for_guild(ctx, guild_id).await;
    Ok(())
}
