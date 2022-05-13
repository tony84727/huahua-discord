use crate::{
    audio::{
        join_channel, stop_for_guild, try_join_authors_channel, try_parse_voice_channel_id,
        try_play_file, try_play_ytdl, PlayError,
    },
    discord::{check_serenity_result, MessageWrapper},
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
    let guild_id = match msg.guild_id {
        Some(id) => id,
        None => {
            return Ok(());
        }
    };
    let channel_id = args.single::<String>().ok();
    match channel_id {
        Some(id) => match try_parse_voice_channel_id(ctx, &id).await {
            Some(id) => {
                if let Err(why) = join_channel(ctx, guild_id, id).await {
                    check_serenity_result(msg.reply(ctx, "本毛無法加入您的頻道").await);
                    log::error!("fail to join the channel, err: {:?}", why);
                }
            }
            None => {
                check_serenity_result(msg.reply(ctx, "無效的id").await);
                return Ok(());
            }
        },
        None => {
            try_join_authors_channel(ctx, MessageWrapper(ctx, msg)).await;
        }
    }

    Ok(())
}

#[command]
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_serenity_result(msg.reply(&ctx.http, "用法!play <URL>").await);
            return Ok(());
        }
    };
    let guild = msg.guild(&ctx.cache).unwrap();
    let guild_id = guild.id;
    try_join_authors_channel(ctx, MessageWrapper(ctx, msg)).await;
    if let Err(why) = try_play_ytdl(ctx, msg, &url, guild_id).await {
        log::error!("fail to play {:?}", why);
    }
    Ok(())
}

#[command]
async fn tbc(ctx: &Context, msg: &Message) -> CommandResult {
    try_join_authors_channel(ctx, MessageWrapper(ctx, msg)).await;
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
    try_join_authors_channel(ctx, MessageWrapper(ctx, msg)).await;
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
