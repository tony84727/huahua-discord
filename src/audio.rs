use serenity::{
    client::Context,
    model::{
        channel::{Channel, ChannelType, Message},
        id::{ChannelId, GuildId},
    },
};
use songbird::input::Input;
use std::{
    ffi::OsStr,
    fmt::Debug,
    io::{Read, Seek, SeekFrom},
};

use crate::discord::{check_serenity_result, AuthorVoiceChannelFinder, Replyable};

pub fn mp3_to_songbird_input<R: Read + Seek + Send + Sync + 'static>(source: R) -> Input {
    let decoder = rodio::Decoder::new_mp3(source).unwrap();
    let source = RodioMediaSource { decoder };
    let reader = Reader::Extension(Box::new(source));
    Input::new(true, reader, Codec::Pcm, Container::Raw, None)
}

struct RodioMediaSource<R>
where
    R: Read + Seek + Send + Sync,
{
    decoder: rodio::Decoder<R>,
}

impl<R> MediaSource for RodioMediaSource<R>
where
    R: Read + Seek + Send + Sync,
{
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

impl<R> Seek for RodioMediaSource<R>
where
    R: Read + Seek + Send + Sync,
{
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "unsupported",
        ))
    }
}

impl<R> Read for RodioMediaSource<R>
where
    R: Read + Seek + Send + Sync,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let sample_count = buf.len() / 2;
        let mut count = 0;
        for _ in 0..sample_count {
            let sample = self.decoder.next();
            match sample {
                None => {
                    break;
                }
                Some(sample) => {
                    for byte in sample.to_ne_bytes().into_iter() {
                        buf[count] = byte;
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }
}

pub async fn join_channel(
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

pub async fn try_join_authors_channel<I: Replyable + AuthorVoiceChannelFinder>(
    ctx: &Context,
    intent: I,
) {
    match intent.find_user_voice_channel().await {
        Ok(Some((guild_id, channel_id))) => match join_channel(ctx, guild_id, channel_id).await {
            Ok(()) => (),
            Err(err) => {
                log::error!("fail to join voice channel, {:?}", err)
            }
        },
        Ok(None) => {
            check_serenity_result(intent.reply(format!("您沒有在任何語音頻道").as_str()).await);
        }
        Err(why) => {
            log::error!("fail to find user's voice channel, {:?}", why);
        }
    }
}

pub async fn try_parse_voice_channel_id(ctx: &Context, id: &str) -> Option<ChannelId> {
    let channel_id = match ChannelId::from_str(ctx, &id) {
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

#[derive(Debug, PartialEq)]
pub enum PlayError {
    NotInChannel,
    CannotPlay,
}

pub async fn try_play_source(
    ctx: &Context,
    guild_id: GuildId,
    source: Input,
) -> Result<(), PlayError> {
    let manager = songbird::get(ctx).await.unwrap();
    match manager.get(guild_id) {
        Some(handler_lock) => {
            let mut handler = handler_lock.lock().await;
            handler.play_input(source);
            Ok(())
        }
        None => Err(PlayError::NotInChannel),
    }
}

pub async fn try_play_ytdl(
    ctx: &Context,
    msg: &Message,
    url: &str,
    guild_id: GuildId,
) -> Result<(), PlayError> {
    let source = match songbird::ytdl(&url).await {
        Ok(source) => source,
        Err(why) => {
            log::error!("cannot play youtube, url: {:?}", why);
            check_serenity_result(msg.reply(&ctx.http, "無法播放QAQ").await);
            return Ok(());
        }
    };
    try_play_source(ctx, guild_id, source).await
}

pub async fn try_play_file<P: AsRef<OsStr> + Debug>(
    ctx: &Context,
    guild_id: GuildId,
    path: P,
) -> Result<(), PlayError> {
    try_play_source(ctx, guild_id, songbird::input::File::new(&path)).await
}

pub async fn stop_for_guild(ctx: &Context, guild_id: GuildId) {
    let manager = songbird::get(ctx).await.expect("cannot get songbird");
    if let Some(call) = manager.get(guild_id) {
        call.lock().await.stop()
    }
}
