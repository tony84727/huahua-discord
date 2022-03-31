use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serenity::model::id::UserId;
use serenity::prelude::TypeMapKey;
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

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
pub trait Store: Sync + Send {
    type Output: Read;
    async fn get(&self, key: &str) -> Result<Self::Output, StoreGetError>;
    async fn put<R: Read + Send + Unpin>(
        &self,
        key: &str,
        mut data: R,
    ) -> Result<(), StorePutError>;
}

pub struct LocalStore {
    dir: PathBuf,
}

#[async_trait]
impl Store for LocalStore {
    type Output = File;
    async fn get(&self, key: &str) -> Result<Self::Output, StoreGetError> {
        let file = self.dir.join(key);
        File::open(file).map_err(|err| match err {
            err if err.kind() == io::ErrorKind::NotFound => StoreGetError::NotFound,
            err => StoreGetError::IO(err),
        })
    }

    async fn put<R: Read + Send + Unpin>(
        &self,
        key: &str,
        mut data: R,
    ) -> Result<(), StorePutError> {
        let file = self.dir.join(key);
        let exists = Self::is_file_exists(&file).map_err(StorePutError::IO)?;
        if exists {
            return Err(StorePutError::AlreadyExist);
        }
        let mut file = File::open(file).map_err(StorePutError::IO)?;
        std::io::copy(&mut data, &mut file)
            .map(|_| ())
            .map_err(StorePutError::IO)
    }
}

impl LocalStore {
    pub fn new<P: Into<PathBuf>>(dir: P) -> Self {
        Self { dir: dir.into() }
    }
    fn is_file_exists<P: AsRef<Path>>(path: P) -> io::Result<bool> {
        match File::open(path) {
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

impl MediaOrigin {
    fn cache_key(&self) -> String {
        let mut input = vec![];
        input.extend_from_slice(self.url.as_bytes());
        input.extend_from_slice(&self.start.as_secs().to_ne_bytes());
        input.extend_from_slice(&self.length.as_secs().to_ne_bytes());
        format!("{:?}", md5::compute(input))
    }
}

#[derive(Serialize, Deserialize)]
pub struct Fx {
    pub name: String,
    pub description: String,
    pub author: UserId,
    pub origin: MediaOrigin,
}

pub enum RepositoryAddError {
    IO(mongodb::error::Error),
}

pub enum RepositoryGetError {
    IO(mongodb::error::Error),
    NotFound,
}

#[async_trait]
pub trait Repository: Send + Sync {
    async fn add(&self, fx: Fx) -> Result<(), RepositoryAddError>;
    async fn get(&self, name: &str) -> Result<Fx, RepositoryGetError>;
}

pub struct MongoDBRepository {
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

impl MongoDBRepository {
    pub fn new(client: mongodb::Database) -> Self {
        Self { client }
    }
}

#[async_trait]
pub trait Creator: Send + Sync {
    type Output: std::io::Read;
    type Error: Debug + Send + Sync;
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

pub struct PreviewingFx {
    pub media: Vec<u8>,
    pub fx: Fx,
}

pub struct Controller<C, S, R>
where
    C: Creator,
    S: Store + 'static,
    R: Repository,
{
    creator: Arc<C>,
    store: Arc<S>,
    repository: Arc<R>,
}

impl<C, S, R> Controller<C, S, R>
where
    C: Creator,
    S: Store + 'static,
    R: Repository,
{
    pub fn new(creator: C, store: S, repository: R) -> Self {
        Self {
            creator: Arc::new(creator),
            store: Arc::new(store),
            repository: Arc::new(repository),
        }
    }
    pub async fn init_create_fx(&self, fx: Fx) -> Result<PreviewingFx, C::Error> {
        let mut output = self.creator.create(&fx.origin).await?;
        let mut buf = vec![];
        output.read_to_end(&mut buf).unwrap();
        let store = self.store.clone();
        let to_store = buf.clone();
        let key = fx.origin.cache_key();
        tokio::spawn(async move {
            if let Err(why) = store.put(&key, &*to_store).await {
                log::error!("fail to store fx, err: {:?}", why);
            }
        });
        Ok(PreviewingFx { fx, media: buf })
    }

    pub async fn confirm_create(&self, preview: PreviewingFx) -> Result<(), RepositoryAddError> {
        self.repository.add(preview.fx).await
    }
}

pub struct FxController;

impl TypeMapKey for FxController {
    type Value = Controller<YoutubeDLCreator, LocalStore, MongoDBRepository>;
}
