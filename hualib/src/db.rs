use mongodb::{error, options::ClientOptions, Client};

pub async fn new_connection() -> error::Result<Client> {
    let options = ClientOptions::parse("mongodb://localhost:27017").await?;
    Client::with_options(options)
}
