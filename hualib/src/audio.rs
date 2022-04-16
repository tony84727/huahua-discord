use serenity::{
    cache::FromStrAndCache,
    client::Context,
    model::{
        channel::{Channel, ChannelType, Message},
        id::{ChannelId, GuildId},
    },
};
use songbird::input::{reader::MediaSource, Codec, Container, Input, Reader};
use std::{
    ffi::OsStr,
    fmt::Debug,
    io::{Read, Seek, SeekFrom},
};

use crate::discord::check_msg;

#[allow(dead_code)]
fn mp3_to_songbird_input<R: Read + Seek + Send + Sync + 'static>(source: R) -> Input {
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

pub async fn try_join_channel(ctx: &Context, msg: &Message, channel_id: Option<ChannelId>) {
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
            handler.play_only_source(source);
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
            check_msg(msg.reply(&ctx.http, "無法播放QAQ").await);
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
    let source = match songbird::ffmpeg(&path).await {
        Ok(input) => input,
        Err(err) => {
            log::error!("cannot play {:?} sound effect, {:?}", path, err);
            return Err(PlayError::CannotPlay);
        }
    };
    try_play_source(ctx, guild_id, source).await
}
