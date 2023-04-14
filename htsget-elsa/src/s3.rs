use crate::Error::{DeserializeError, GetObjectError, PutObjectError, SerializeError};
use crate::{Cache, Error, GetObject, Result};
use async_trait::async_trait;
use aws_sdk_s3::primitives::{ByteStream, DateTime};
use aws_sdk_s3::Client;
use bytes::Bytes;
use htsget_config::resolver::Resolver;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, to_string, to_vec};
use std::ops::Add;
use std::time::{Duration, SystemTime};
use tracing::instrument;

#[derive(Debug)]
pub struct S3 {
    s3_client: Client,
    cache_bucket: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CacheItem {
    item: Vec<Resolver>,
    max_age: u64,
}

impl S3 {
    pub fn new(s3_client: Client, cache_bucket: String) -> Self {
        Self {
            s3_client,
            cache_bucket,
        }
    }

    pub async fn new_with_default_config(cache_bucket: String) -> Self {
        Self::new(
            Client::new(&aws_config::load_from_env().await),
            cache_bucket,
        )
    }
}

impl S3 {
    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        bucket: impl Into<String> + Send,
        key: impl Into<String> + Send,
    ) -> Result<(T, Option<DateTime>)> {
        let output = self
            .s3_client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|err| GetObjectError(err.to_string()))?;

        let last_modified = output.last_modified().copied();

        let output = output
            .body
            .collect()
            .await
            .map_err(|err| GetObjectError(err.to_string()))?
            .into_bytes();

        Ok((
            from_slice(output.as_ref()).map_err(|err| DeserializeError(err.to_string()))?,
            last_modified,
        ))
    }
}

#[async_trait]
impl GetObject for S3 {
    type Error = Error;

    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        bucket: impl Into<String> + Send,
        key: impl Into<String> + Send,
    ) -> Result<T> {
        Ok(self.get_object(bucket, key).await?.0)
    }
}

#[async_trait]
impl Cache for S3 {
    type Error = Error;
    type Item = Vec<Resolver>;

    #[instrument(level = "trace", skip_all, ret)]
    async fn get<K: AsRef<str> + Send + Sync>(&self, key: K) -> Result<Option<Self::Item>> {
        let (object, last_modified): (CacheItem, _) = self
            .get_object(self.cache_bucket.clone(), key.as_ref())
            .await?;

        match last_modified {
            Some(last_modified)
                if last_modified.as_nanos()
                    <= DateTime::from(
                        SystemTime::now().add(Duration::from_secs(object.max_age)),
                    )
                    .as_nanos() =>
            {
                Ok(Some(object.item))
            }
            _ => Ok(None),
        }
    }

    #[instrument(level = "trace", skip_all, ret)]
    async fn put<K: AsRef<str> + Send + Sync>(
        &self,
        key: K,
        item: Self::Item,
        max_age: u64,
    ) -> Result<()> {
        self.s3_client
            .put_object()
            .bucket(self.cache_bucket.clone())
            .key(key.as_ref())
            .body(ByteStream::from(Bytes::from(
                to_vec(&CacheItem { item, max_age })
                    .map_err(|err| SerializeError(err.to_string()))?,
            )))
            .send()
            .await
            .map_err(|err| PutObjectError(err.to_string()))?;

        Ok(())
    }
}
