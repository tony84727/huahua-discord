use serenity::{
    client::Context,
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::Message,
        id::{ChannelId, GuildId},
    },
};

#[command]
pub async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    match find_voice_channel_of_user(ctx, msg).await {
        Some((guild_id, channel_id)) => match play_in_channel(ctx, guild_id, channel_id).await {
            Ok(()) => (),
            Err(err) => {
                log::error!("fail to join voice channel, {:?}", err)
            }
        },
        None => {
            msg.reply(ctx, format!("您沒有在任何語音頻道")).await?;
        }
    }
    CommandResult::Ok(())
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
    let manager = songbird::get(ctx).await.unwrap();
    if let Some(handler_lock) = manager.get(guild_id) {
        let mut handler = handler_lock.lock().await;
        let source = match songbird::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                log::error!("cannot play youtube, url: {:?}", why);
                check_msg(msg.reply(&ctx.http, "無法播放QAQ").await);
                return Ok(());
            }
        };
        handler.play_only_source(source);
        Ok(())
    } else {
        join(ctx, msg, args).await
    }
}

fn check_msg(result: serenity::Result<Message>) {
    if let Err(err) = result {
        log::error!("error sending message: {:?}", err);
    }
}

async fn play_in_channel(
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
