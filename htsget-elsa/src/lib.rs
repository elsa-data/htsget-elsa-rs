pub mod dynamodb;
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
    InvalidUri(String),
    #[error("failed to get manifest from Elsa: `{0}`")]
    GetGetManifest(reqwest::Error),
    #[error("failed to deserialize type: `{0}")]
    DeserializeError(String),
    #[error("failed to get object from storage: `{0}`")]
    GetObjectError(String),
}

#[async_trait]
pub trait Cache {
    type Item;

    async fn get<K: AsRef<str> + Send>(&self, key: K) -> Self::Item;
    async fn put<K: AsRef<str> + Send>(&self, key: K, item: Self::Item, expirey: u64);
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
pub trait ResolverFromElsa {
    type Error;

    async fn try_get(&self, release_key: String) -> result::Result<Resolver, Self::Error>;
}