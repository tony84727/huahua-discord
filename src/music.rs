use std::collections::VecDeque;

use serenity::{
    client::Context,
    framework::standard::CommandResult,
    model::{channel::Message, guild::Guild},
};

pub struct Song {
    url: String,
}

pub struct Player {
    queue: VecDeque<Song>,
}

impl Player {
    pub fn queue(&mut self, song: Song) {
        self.queue.push_back(song);
    }

    pub fn play(&mut self) {}
}

/// Handle music related interaction
pub struct Controller;

impl Controller {
    pub fn new() -> Self {
        Self
    }

    pub async fn join(&self, ctx: &Context, msg: &Message) -> CommandResult {
        // self.guild.voice_states.get(msg.)
        let author = msg.author;
        msg.guild(ctx.cache).await.unwrap().voice_states.get(author)
    }
}
