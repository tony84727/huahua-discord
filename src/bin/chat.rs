use huahua_discord::config;
use regex::Regex;
use serenity::{client::Client, framework::StandardFramework};
use std::io::Write;
use std::sync::mpsc::{self, Receiver, Sender};

struct InteractivePrompt {
    prompt: String,
    sender: Sender<Result<String, std::io::Error>>,
}

impl InteractivePrompt {
    fn new(prompt: String) -> (Self, Receiver<Result<String, std::io::Error>>) {
        let (sender, receiver) = mpsc::channel();
        (Self { prompt, sender }, receiver)
    }

    fn run(&self) {
        self.prompt();
        for line in std::io::stdin().lines() {
            if line.is_ok() {
                self.prompt();
            }
            self.sender.send(line);
        }
    }

    fn write(&self) {
        Self::clean_line();
        println!();
        self.prompt();
    }

    fn prompt(&self) {
        print!("{}", self.prompt);
        std::io::stdout().flush().unwrap();
    }

    fn clean_line() {
        todo!()
    }
}

#[derive(PartialEq, Debug)]
struct ParsedCommand {
    command: String,
    args: String,
}

struct CommandParser {
    command_pattern: regex::Regex,
}

impl CommandParser {
    fn new() -> Self {
        Self {
            command_pattern: Regex::new(r"^%(?P<command>\S+)\s+(?P<args>.*)").unwrap(),
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
            let args = match capture.name("args") {
                Some(args) => args.as_str().to_string(),
                None => {
                    return None;
                }
            };
            Some(ParsedCommand { command, args })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("%switch 123123" => Some(ParsedCommand{command: "switch".to_string(), args: "123123".to_string()}))]
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
    let (prompt, receiver) = InteractivePrompt::new(">".to_string());
    tokio::spawn(async move {
        prompt.run();
    });
    for line in receiver.into_iter() {
        let line = line.unwrap();
        eprintln!("in: {}", line);
    }
}
