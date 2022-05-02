use async_trait::async_trait;
use serenity::{
    client::Context,
    model::{
        channel::Message,
        id::{ChannelId, GuildId},
        interactions::application_command::ApplicationCommandInteraction,
    },
};

pub fn check_serenity_result<T>(result: serenity::Result<T>) {
    if let Err(err) = result {
        log::error!("error sending message: {:?}", err);
    }
}

#[async_trait]
pub trait Replyable {
    async fn reply(&self, content: &str) -> Result<(), serenity::Error>;
}

pub struct MessageWrapper<'a>(pub &'a Context, pub &'a Message);

#[async_trait]
impl<'a> Replyable for MessageWrapper<'a> {
    async fn reply(&self, content: &str) -> Result<(), serenity::Error> {
        self.1.reply(self.0, content).await.map(|_| ())
    }
}

pub struct InteractionWrapper<'a>(pub &'a Context, pub &'a ApplicationCommandInteraction);

#[async_trait]
impl<'a> Replyable for InteractionWrapper<'a> {
    async fn reply(&self, content: &str) -> Result<(), serenity::Error> {
        self.1
            .create_followup_message(self.0, |response| response.content(content))
            .await
            .map(|_| ())
    }
}

#[async_trait]
pub trait AuthorVoiceChannelFinder {
    async fn find_user_voice_channel(
        &self,
    ) -> Result<Option<(GuildId, ChannelId)>, serenity::Error>;
}

#[async_trait]
impl<'a> AuthorVoiceChannelFinder for InteractionWrapper<'a> {
    async fn find_user_voice_channel(
        &self,
    ) -> Result<Option<(GuildId, ChannelId)>, serenity::Error> {
        if let Some(member) = &self.1.member {
            if let Some(guild_id) = self.1.guild_id {
                if let Some(guild) = self.0.cache.guild(guild_id) {
                    return Ok(guild
                        .voice_states
                        .get(&member.user.id)
                        .and_then(|state| state.channel_id)
                        .map(|channel_id| (guild_id, channel_id)));
                }
            }
        }
        Ok(None)
    }
}

#[async_trait]
impl<'a> AuthorVoiceChannelFinder for MessageWrapper<'a> {
    async fn find_user_voice_channel(
        &self,
    ) -> Result<Option<(GuildId, ChannelId)>, serenity::Error> {
        Ok(self.1.guild(&self.0.cache).and_then(|guild| {
            guild
                .voice_states
                .get(&self.1.author.id)
                .and_then(|voice_state| voice_state.channel_id)
                .map(|channel_id| (guild.id, channel_id))
        }))
    }
}
