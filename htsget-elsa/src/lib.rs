use std::result;

use async_trait::async_trait;
use htsget_config::resolver::Resolver;
use serde::Deserialize;
use thiserror::Error;

pub mod elsa_endpoint;
pub mod s3;
#[cfg(feature = "test-utils")]
pub mod test_utils;

pub type Result<T> = result::Result<T, Error>;

/// Main error type for this crate.
#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid client: `{0}`")]
    InvalidClient(reqwest::Error),
    #[error("invalid uri constructed from release key: `{0}`")]
    InvalidReleaseUri(String),
    #[error("failed to get manifest from Elsa: `{0}`")]
    GetManifest(String),
    #[error("failed to deserialize: `{0}")]
    DeserializeError(String),
    #[error("failed to serialize: `{0}")]
    SerializeError(String),
    #[error("failed to get object from storage: `{0}`")]
    GetObjectError(String),
    #[error("failed to put object into storage: `{0}`")]
    PutObjectError(String),
    #[error("invalid uri received from manifest: `{0}`")]
    InvalidManifest(String),
    #[error("unsupported component of manifest: `{0}`")]
    UnsupportedManifestFeature(String),
    #[error("system error: `{0}`")]
    SystemError(String),
}

/// Cache resolver objects to a cache.
#[async_trait]
pub trait Cache {
    type Error;
    type Item;

    /// Get the resolvers from the cache.
    async fn get<K: AsRef<str> + Send + Sync>(
        &self,
        key: K,
    ) -> result::Result<Option<Self::Item>, Self::Error>;

    /// Put the resolvers in the cache.
    async fn put<K: AsRef<str> + Send + Sync>(
        &self,
        key: K,
        item: Self::Item,
        max_age: u64,
    ) -> result::Result<(), Self::Error>;
}

/// Get objects from cloud storage.
#[async_trait]
pub trait GetObject {
    type Error;

    /// Get the object.
    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        bucket: impl Into<String> + Send,
        key: impl Into<String> + Send,
    ) -> result::Result<T, Self::Error>;
}

/// Get resolvers from Elsa.
#[async_trait]
pub trait ResolversFromElsa {
    type Error;

    /// Get the resolvers from Elsa using the release key.
    async fn try_get(&self, release_key: String) -> result::Result<Vec<Resolver>, Self::Error>;
}
