use chrono::serde::ts_seconds::{deserialize as from_ts, serialize as to_ts};
use chrono::{DateTime, Utc};
use mongodb::bson::doc;
use serde::{Deserialize, Serialize};
use serenity::model::id::{GuildId, InteractionId, UserId};
use serenity::model::interactions::application_command::ApplicationCommandInteraction;
use songbird::input::ChildContainer;
use std::fmt::Debug;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::ioutils::TappableReader;

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
        let mut file = File::create(file).map_err(StorePutError::IO)?;
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiscordOrigin {
    pub guild: Option<GuildId>,
    pub interaction: InteractionId,
    pub author: Option<UserId>,
    #[serde(serialize_with = "to_ts", deserialize_with = "from_ts")]
    pub drafted_at: DateTime<Utc>,
}

impl From<ApplicationCommandInteraction> for DiscordOrigin {
    fn from(interaction: ApplicationCommandInteraction) -> Self {
        Self {
            guild: interaction.guild_id,
            interaction: interaction.id,
            author: interaction.member.map(|member| member.user.id),
            drafted_at: Utc::now(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Fx {
    pub name: String,
    pub description: String,
    pub discord: DiscordOrigin,
    pub media: MediaOrigin,
}

#[derive(Debug)]
pub enum RepositoryAddError {
    IO(mongodb::error::Error),
    AlreadyExists,
}

#[derive(Debug)]
pub enum RepositoryGetError {
    IO(mongodb::error::Error),
    NotFound,
}

#[async_trait]
pub trait Repository: Send + Sync {
    async fn add_draft(&self, fx: Fx) -> Result<(), RepositoryAddError>;
    async fn add(&self, fx: Fx) -> Result<(), RepositoryAddError>;
    async fn get(&self, identity: &FxIdentity) -> Result<Fx, RepositoryGetError>;
}

pub struct MongoDBRepository {
    client: mongodb::Database,
}

#[async_trait]
impl Repository for MongoDBRepository {
    async fn add_draft(&self, fx: Fx) -> Result<(), RepositoryAddError> {
        self.client
            .collection("fx_drafts")
            .insert_one(fx, None)
            .await
            .map(|_| ())
            .map_err(RepositoryAddError::IO)
    }
    async fn add(&self, fx: Fx) -> Result<(), RepositoryAddError> {
        self.client
            .collection("fx")
            .insert_one(fx, None)
            .await
            .map(|_| ())
            .map_err(RepositoryAddError::IO)
    }

    async fn get(&self, identity: &FxIdentity) -> Result<Fx, RepositoryGetError> {
        let FxIdentity(guild_id, name) = identity;
        let condition = doc! {
            "name": name,
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
    type Output = BufReader<ChildContainer>;
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
        Ok(
            match songbird::input::children_to_reader::<u8>(vec![ytdl, ffmpeg]) {
                songbird::input::Reader::Pipe(buf_reader) => buf_reader,
                _ => panic!("unexpected"),
            },
        )
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

#[derive(Debug)]
pub enum CachedCreatorError<StoreError, CreateError>
where
    StoreError: Debug,
    CreateError: Debug,
{
    Cache(StoreError),
    Create(CreateError),
}

pub struct CachedCreator<C: Creator, S: Store> {
    creator: Arc<C>,
    store: Arc<S>,
}

#[async_trait]
impl<C, S> Creator for CachedCreator<C, S>
where
    C: Creator,
    S: Store + 'static,
    S::Output: Sync + Send + 'static,
    C::Output: Sync + Send + Unpin + 'static,
    C::Error: Sync + Send,
{
    type Output = Box<dyn Read>;

    type Error = CachedCreatorError<StoreGetError, C::Error>;

    async fn create(&self, origin: &MediaOrigin) -> Result<Self::Output, Self::Error> {
        let key = origin.cache_key();
        match self.store.get(&key).await {
            Ok(media) => Ok(Box::new(media)),
            Err(StoreGetError::NotFound) => {
                let output = self
                    .creator
                    .create(origin)
                    .await
                    .map_err(CachedCreatorError::Create)?;
                // let to_store = output.clone();
                // let reader =
                let mut reader = TappableReader::new(output);
                let to_store = reader.tap();
                let store = self.store.clone();
                tokio::spawn(async move { store.put(&key, to_store).await });
                Ok(Box::new(reader))
            }
            Err(why) => Err(CachedCreatorError::Cache(why)),
        }
    }
}

impl<C, S> CachedCreator<C, S>
where
    C: Creator,
    S: Store,
{
    pub fn new(creator: C, store: S) -> Self {
        Self {
            creator: Arc::new(creator),
            store: Arc::new(store),
        }
    }
}

pub struct PreviewingFx {
    pub media: Vec<u8>,
    pub fx: Fx,
}

#[derive(Debug)]
pub struct FxIdentity(pub GuildId, pub String);

pub struct FxWithMedia(pub Fx, pub Vec<u8>);

#[derive(Debug)]
pub enum GetFxError<C> {
    Repository(RepositoryGetError),
    Create(C),
}

pub struct Controller<C, R>
where
    C: Creator,
    R: Repository,
{
    creator: Arc<C>,
    repository: Arc<R>,
}

impl<C, R> Controller<C, R>
where
    C: Creator,
    R: Repository,
{
    pub fn new(creator: C, repository: R) -> Self {
        Self {
            creator: Arc::new(creator),
            repository: Arc::new(repository),
        }
    }
    pub async fn init_create_fx(&self, fx: Fx) -> Result<PreviewingFx, C::Error> {
        let mut output = self.creator.create(&fx.media).await?;
        let mut buf = vec![];
        output.read_to_end(&mut buf).unwrap();
        Ok(PreviewingFx { fx, media: buf })
    }

    pub async fn confirm_create(&self, fx: Fx) -> Result<(), RepositoryAddError> {
        self.repository.add(fx).await
    }
    pub async fn get(&self, identity: &FxIdentity) -> Result<FxWithMedia, GetFxError<C::Error>> {
        let fx = self
            .repository
            .get(identity)
            .await
            .map_err(GetFxError::Repository)?;
        let mut media = self
            .creator
            .create(&fx.media)
            .await
            .map_err(GetFxError::Create)?;
        let mut buf = vec![];
        media.read_to_end(&mut buf).unwrap();
        Ok(FxWithMedia(fx, buf))
    }
}
