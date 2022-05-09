use huahua_discord::config;
use regex::Regex;
use serenity::model::id::{ChannelId, GuildId};
use serenity::{client::Client, framework::StandardFramework};
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{self, BufRead, Write};

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

#[derive(Default, Debug)]
struct ChatContext {
    current_channel: Option<ChannelIdentity>,
}

struct ChatConsole {
    context: ChatContext,
    prompt: InteractivePrompt,
    parser: CommandParser,
    commander: Commands,
}

#[derive(PartialEq, Eq, Debug)]
struct ChannelIdentity(GuildId, ChannelId);

/// Chat console's command
trait Command: Debug {
    fn exec(&self, args: Option<String>);
}

#[derive(Debug)]
struct SelectCommand<'c> {
    context: &'c mut ChatContext,
}

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

impl<'c> Command for SelectCommand<'c> {
    fn exec(&self, args: Option<String>) {
        let args = match args {
            Some(args) => args,
            None => {
                self.print_usage();
                return;
            }
        };
        match parse_channel_identity(&args) {
            Some(identity) => {
                self.context.current_channel = Some(identity);
                println!("switch to channel {}/{}", identity.0, identity.1);
            }
            None => {
                println!("cannot parse channel identity");
                self.print_usage();
            }
        }
    }
}

impl<'c> SelectCommand<'c> {
    fn print_usage(&self) {
        println!("please specify a channel identity(guildId/channelId). usage: %select <guildId>/<channelId>");
    }
}

// struct Commander {
//     commands: HashMap<String, Box<dyn Command>>,
// }




enum Commands {
    Use(SelectCommand),
}

struct Commander {
    commands: Commands
}

impl Commander {
    fn find_command(&self, name: &str) -> Option<&Box<dyn Command>> {
        match name {
            "use" => SelectCommand
        }
    }
}


impl ChatConsole {
    fn new() -> Self {
        let mut instance = Self {
            prompt: InteractivePrompt::new(">".to_string()),
            parser: CommandParser::new(),
            context: Default::default(),
            commander: Commands::new(),
        };
        let select_command = SelectCommand {
            context: &mut instance.context,
        };
        instance.commander.register_command("use", select_command);
        instance
    }
    fn run(self) -> io::Result<()> {
        for line in self.prompt.into_iter() {
            let line = line?;
            match self.parser.parse(&line) {
                Some(ParsedCommand { command, args }) => match command {
                    match self.commander.find_command(&command) {
                        Some(command) => {
                            command.exec(args),
                        }
                        None => {
                            todo!();
                        }
                    }
                },
                None => log::info!("received {}", line),
            };
        }
        Ok(())
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

    #[derive(Clone, PartialEq, Eq, Debug)]
    struct DummyCommand(String);

    impl Command for DummyCommand {
        fn exec(&self, args: Option<String>) {}
    }

    // #[test]
    // fn test_commander_find_command() {
    //     let mut commander = Commander::new();
    //     let command = DummyCommand("a".to_string());
    //     commander.register_command("a", command.clone());
    //     assert_eq!(&command, commander.find_command("a").unwrap().as_ref());
    // }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_default_env()
        .filter_module("hualib", log::LevelFilter::Debug)
        .init();
    let bot_config = config::Bot::load().await.expect("fail to load bot config");
    let mut client = Client::builder(bot_config.token)
        .framework(StandardFramework::default())
        .await
        .expect("error while creating client");
    let console = ChatConsole::new();
    console.run().unwrap();
}
