use async_trait::async_trait;
use huahua_discord::config;
use regex::Regex;
use serenity::client::{Context, EventHandler};
use serenity::model::id::{ChannelId, GuildId};
use serenity::{client::Client, framework::StandardFramework};
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{self, BufRead, Write};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

struct InteractivePrompt {
    prompt: String,
}

impl Iterator for InteractivePrompt {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.prompt();
        std::io::stdin().lock().lines().next()
    }
}

impl InteractivePrompt {
    fn new(prompt: String) -> Self {
        Self { prompt }
    }

    fn prompt(&self) {
        print!("{}", self.prompt);
        std::io::stdout().flush().unwrap();
    }
}

#[derive(PartialEq, Debug)]
struct ParsedCommand {
    command: String,
    args: Option<String>,
}

struct CommandParser {
    command_pattern: regex::Regex,
}

impl CommandParser {
    fn new() -> Self {
        Self {
            command_pattern: Regex::new(r"^%(?P<command>\S+)(\s+(?P<args>.*))?").unwrap(),
        }
    }

    fn parse(&self, input: &str) -> Option<ParsedCommand> {
        self.command_pattern.captures(input).and_then(|capture| {
            let command = match capture.name("command") {
                Some(command) => command.as_str().to_string(),
                None => {
                    return None;
                }
            };
            let args = capture.name("args").map(|arg| arg.as_str().to_string());
            Some(ParsedCommand { command, args })
        })
    }
}

struct ChatContext {
    client: Context,
    current_channel: Option<ChannelIdentity>,
}

impl ChatContext {
    fn new(client: Context) -> Self {
        Self {
            client,
            current_channel: None,
        }
    }
}

struct ChatConsole {
    context: ChatContext,
    parser: CommandParser,
    commander: Commander,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct ChannelIdentity(GuildId, ChannelId);

/// Chat console's command
trait Command: Debug {
    fn exec(&self, ctx: &mut ChatContext, args: Option<String>);
}

#[derive(Debug)]
struct SelectCommand;

fn consume_and_parse_u64<'a, I: Iterator<Item = &'a str>>(iterator: &mut I) -> Option<u64> {
    let str = iterator.next();
    match str {
        Some(number) => match number.parse() {
            Ok(number) => Some(number),
            Err(_) => None,
        },
        None => None,
    }
}

fn parse_channel_identity(input: &str) -> Option<ChannelIdentity> {
    let mut segments = input.split(' ').map(|x| x.trim());
    let guild_id = match consume_and_parse_u64(&mut segments) {
        Some(n) => n,
        None => {
            return None;
        }
    };
    let channel_id = match consume_and_parse_u64(&mut segments) {
        Some(n) => n,
        None => {
            return None;
        }
    };
    Some(ChannelIdentity(GuildId(guild_id), ChannelId(channel_id)))
}

impl Command for SelectCommand {
    fn exec(&self, ctx: &mut ChatContext, args: Option<String>) {
        let args = match args {
            Some(args) => args,
            None => {
                self.print_usage();
                return;
            }
        };
        match parse_channel_identity(&args) {
            Some(identity) => {
                ctx.current_channel = Some(identity);
                let ChannelIdentity(guild_id, channel_id) = identity;
                println!("switch to channel {}/{}", guild_id, channel_id);
            }
            None => {
                println!("cannot parse channel identity");
                self.print_usage();
            }
        }
    }
}

impl SelectCommand {
    fn print_usage(&self) {
        println!("please specify a channel identity(guildId/channelId). usage: %select <guildId>/<channelId>");
    }
}

struct Commander {
    commands: HashMap<String, Box<dyn Command + Send + Sync>>,
}

impl Commander {
    fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    fn register_command<C: Command + Send + Sync + 'static>(&mut self, name: &str, command: C) {
        self.commands.insert(name.to_string(), Box::new(command));
    }

    fn find_command(&self, name: &str) -> Option<&Box<dyn Command + Send + Sync>> {
        self.commands.get(name)
    }
}

impl ChatConsole {
    fn new(context: Context) -> Self {
        let mut instance = Self {
            parser: CommandParser::new(),
            context: ChatContext::new(context),
            commander: Commander::new(),
        };
        instance.commander.register_command("use", SelectCommand);
        instance
    }
    async fn run(mut self) -> io::Result<()> {
        let prompt = InteractivePrompt::new(">".to_string());
        for line in prompt.into_iter() {
            let line = line?;
            match self.parser.parse(&line) {
                Some(ParsedCommand { command, args }) => {
                    match self.commander.find_command(&command) {
                        Some(command) => command.exec(&mut self.context, args),
                        None => {}
                    }
                }
                None => {
                    self.chat(line).await;
                }
            };
        }
        Ok(())
    }

    async fn chat(&self, message: String) {
        // self.context.client.cache_and_http.http.send_message(channel_id, map)
        let channel_id = match self.context.current_channel {
            Some(ChannelIdentity(_, channel_id)) => channel_id,
            None => {
                return;
            }
        };
        if let Err(err) = channel_id
            .send_message(&self.context.client, |builder| builder.content(message))
            .await
        {
            log::error!("sending chat: {:?}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("%switch 123123" => Some(ParsedCommand{command: "switch".to_string(), args: Some("123123".to_string())}))]
    #[test_case("%listen" => Some(ParsedCommand{command: "listen".to_string(), args: None}))]
    #[test_case("hello world" => None)]
    fn test_command_parser(input: &str) -> Option<ParsedCommand> {
        let parser = CommandParser::new();
        parser.parse(input)
    }

    #[test_case("123 123" => Some(ChannelIdentity(GuildId(123), ChannelId(123))))]
    fn test_parse_channel_identity(input: &str) -> Option<ChannelIdentity> {
        parse_channel_identity(input)
    }
}

#[derive(Default)]
struct Handler {
    is_loop_running: AtomicBool,
}

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        if !self.is_loop_running.load(Ordering::Relaxed) {
            tokio::spawn(async move {
                let console = ChatConsole::new(ctx);
                console.run().await.unwrap();
            });
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_module("hualib", log::LevelFilter::Debug)
        .init();
    let bot_config = config::Bot::load().await.expect("fail to load bot config");
    let mut client = Client::builder(bot_config.token)
        .event_handler(Handler::default())
        .framework(StandardFramework::default())
        .await
        .expect("error while creating client");
    client.start().await.unwrap();
}
