use huahua_discord::config;
use regex::Regex;
use serenity::model::id::GuildId;
use serenity::{client::Client, framework::StandardFramework};
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

#[derive(Default)]
struct ChatContext {
    current_channel: Option<GuildId>,
}

struct ChatConsole {
    context: ChatContext,
    prompt: InteractivePrompt,
    parser: CommandParser,
}

impl ChatConsole {
    fn new() -> Self {
        Self {
            prompt: InteractivePrompt::new(">".to_string()),
            parser: CommandParser::new(),
            context: Default::default(),
        }
    }
    fn run(self) -> io::Result<()> {
        for line in self.prompt.into_iter() {
            let line = line?;
            match self.parser.parse(&line) {
                Some(command) => println!("invoke command: {}", command.command),
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
