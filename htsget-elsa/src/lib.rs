pub mod elsa_endpoint;
pub mod s3;

use async_trait::async_trait;
use htsget_config::resolver::Resolver;
use serde::Deserialize;
use std::result;
use thiserror::Error;

pub type Result<T> = result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid client: `{0}`")]
    InvalidClient(reqwest::Error),
    #[error("invalid uri constructed from release key: `{0}`")]
    InvalidReleaseUri(String),
    #[error("failed to get manifest from Elsa: `{0}`")]
    GetGetManifest(reqwest::Error),
    #[error("failed to deserialize: `{0}")]
    DeserializeError(String),
    #[error("failed to serialize: `{0}")]
    SerializeError(String),
    #[error("failed to get object from storage: `{0}`")]
    GetObjectError(String),
    #[error("failed to put object into storage: `{0}`")]
    PutObjectError(String),
    #[error("invalid uri received from manifest: `{0}`")]
    InvalidManifestUri(String),
    #[error("unsupported component of manifest: `{0}`")]
    UnsupportedManifestFeature(String),
    #[error("system error: `{0}`")]
    SystemError(String),
}

#[async_trait]
pub trait Cache {
    type Error;
    type Item;

    async fn get<K: AsRef<str> + Send + Sync>(
        &self,
        key: K,
    ) -> result::Result<Option<Self::Item>, Self::Error>;
    async fn put<K: AsRef<str> + Send + Sync>(
        &self,
        key: K,
        item: Self::Item,
        max_age: u64,
    ) -> result::Result<(), Self::Error>;
}

#[async_trait]
pub trait GetObject {
    type Error;

    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        bucket: impl Into<String> + Send,
        key: impl Into<String> + Send,
    ) -> result::Result<T, Self::Error>;
}

#[async_trait]
pub trait ResolversFromElsa {
    type Error;

    async fn try_get(&self, release_key: String) -> result::Result<Vec<Resolver>, Self::Error>;
}
