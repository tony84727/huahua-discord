use std::path::{Path, PathBuf};
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
