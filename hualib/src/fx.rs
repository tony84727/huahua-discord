use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serenity::model::id::UserId;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;
use tokio::{fs, io};

use async_trait::async_trait;

#[derive(Debug)]
pub enum StoreGetError {
    NotFound,
    IO(io::Error),
}

#[derive(Debug)]
pub enum StorePutError {
    AlreadyExist,
    IO(io::Error),
}

#[async_trait]
pub trait Store {
    type Output: io::AsyncRead;
    async fn get(&self, key: &str) -> Result<Self::Output, StoreGetError>;
    async fn put<R: io::AsyncRead + Send + Unpin>(
        &self,
        key: &str,
        mut data: R,
    ) -> Result<(), StorePutError>;
}

struct LocalStore {
    dir: PathBuf,
}

#[async_trait]
impl Store for LocalStore {
    type Output = fs::File;
    async fn get(&self, key: &str) -> Result<Self::Output, StoreGetError> {
        let file = self.dir.join(key);
        fs::File::open(file).await.map_err(|err| match err {
            err if err.kind() == io::ErrorKind::NotFound => StoreGetError::NotFound,
            err => StoreGetError::IO(err),
        })
    }

    async fn put<R: io::AsyncRead + Send + Unpin>(
        &self,
        key: &str,
        mut data: R,
    ) -> Result<(), StorePutError> {
        let file = self.dir.join(key);
        let exists = Self::is_file_exists(&file)
            .await
            .map_err(StorePutError::IO)?;
        if exists {
            return Err(StorePutError::AlreadyExist);
        }
        let mut file = fs::File::open(file).await.map_err(StorePutError::IO)?;
        io::copy(&mut data, &mut file)
            .await
            .map(|_| ())
            .map_err(StorePutError::IO)
    }
}

impl LocalStore {
    async fn is_file_exists<P: AsRef<Path>>(path: P) -> io::Result<bool> {
        match fs::File::open(path).await {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct MediaOrigin {
    pub url: String,
    pub start: Duration,
    pub length: Duration,
}

#[derive(Serialize, Deserialize)]
pub struct Fx {
    pub name: String,
    pub description: String,
    pub author: UserId,
    pub origin: MediaOrigin,
}

enum RepositoryAddError {
    IO(mongodb::error::Error),
}

enum RepositoryGetError {
    IO(mongodb::error::Error),
    NotFound,
}

#[async_trait]
trait Repository {
    async fn add(&self, fx: Fx) -> Result<(), RepositoryAddError>;
    async fn get(&self, name: &str) -> Result<Fx, RepositoryGetError>;
}

struct MongoDBRepository {
    client: mongodb::Database,
}

#[async_trait]
impl Repository for MongoDBRepository {
    async fn add(&self, fx: Fx) -> Result<(), RepositoryAddError> {
        self.client
            .collection("fx")
            .insert_one(fx, None)
            .await
            .map(|_| ())
            .map_err(RepositoryAddError::IO)
    }

    async fn get(&self, name: &str) -> Result<Fx, RepositoryGetError> {
        let condition = doc! {
            "name": name
        };
        match self
            .client
            .collection("fx")
            .find_one(condition, None)
            .await
            .map_err(RepositoryGetError::IO)
        {
            Ok(Some(result)) => Ok(result),
            Ok(None) => Err(RepositoryGetError::NotFound),
            Err(err) => Err(err),
        }
    }
}

#[async_trait]
pub trait Creator {
    type Output: std::io::Read;
    type Error;
    async fn create(&self, origin: &MediaOrigin) -> Result<Self::Output, Self::Error>;
}

#[derive(Debug)]
pub enum YoutubeDLCreateError {
    YoutubeDL(io::Error),
    FFmepg(io::Error),
}

pub struct YoutubeDLCreator;

#[async_trait]
impl Creator for YoutubeDLCreator {
    type Output = songbird::input::Reader;
    type Error = YoutubeDLCreateError;

    async fn create(&self, origin: &MediaOrigin) -> Result<Self::Output, Self::Error> {
        let mut ytdl = Command::new("youtube-dl")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .args([origin.url.as_str(), "-o", "-", "--audio-format", "best"])
            .spawn()
            .map_err(YoutubeDLCreateError::YoutubeDL)?;
        let ytdl_out = ytdl.stdout.take().unwrap();
        let ffmpeg = Self::cut(origin, ytdl_out)
            .await
            .map_err(YoutubeDLCreateError::FFmepg)?;
        Ok(songbird::input::children_to_reader::<u8>(vec![
            ytdl, ffmpeg,
        ]))
    }
}

impl YoutubeDLCreator {
    async fn cut(origin: &MediaOrigin, output: ChildStdout) -> io::Result<Child> {
        Command::new("ffmpeg")
            .stdin(output)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .arg("-ss")
            .arg(format!("{}", origin.start.as_secs()))
            .arg("-t")
            .arg(format!("{}", origin.length.as_secs()))
            .args(&["-i", "-"])
            .arg("-f")
            .arg("mp3")
            .arg("-")
            .spawn()
    }
}
