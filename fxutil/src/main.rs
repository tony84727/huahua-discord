use clap::{Parser, Subcommand};
use hualib::fx::{self, Creator};
use std::{fs, io, time};

#[derive(Parser)]
struct CreateOption {
    #[clap(required = true)]
    url: String,
    #[clap(short = 't', default_value = "0")]
    start: u64,
    #[clap(short = 'l', default_value = "5")]
    length: u64,
}

#[derive(Subcommand)]
enum SubCommands {
    Create(CreateOption),
}

#[derive(Parser)]
struct Option {
    #[clap(subcommand)]
    sub_commands: SubCommands,
}

#[tokio::main]
async fn main() {
    let option = Option::parse();
    match option.sub_commands {
        SubCommands::Create(CreateOption { url, start, length }) => {
            let mut out = fs::File::create("fxout.mp3").unwrap();
            let creator = fx::YoutubeDLCreator;
            let mut result = creator
                .create(&fx::MediaOrigin {
                    start: time::Duration::from_secs(start),
                    length: time::Duration::from_secs(length),
                    url,
                })
                .await
                .unwrap();
            io::copy(&mut result, &mut out).unwrap();
        }
    }
}
