use std::ops::Sub;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use aws_sdk_s3::types::{ByteStream, DateTime};
use aws_sdk_s3::Client;
use bytes::Bytes;
use htsget_config::resolver::Resolver;
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, to_vec};
use tracing::{instrument, trace};

use crate::Error::{DeserializeError, GetObjectError, PutObjectError, SerializeError};
use crate::{Cache, Error, GetObject, Result};

/// S3 storage implementation.
#[derive(Debug)]
pub struct S3 {
    s3_client: Client,
    cache_bucket: String,
}

/// The shape of the item to cache.
#[derive(Debug, Deserialize, Serialize)]
pub struct CacheItem {
    item: Vec<Resolver>,
    max_age: u64,
}

impl S3 {
    /// Create a new S3 storage.
    pub fn new(s3_client: Client, cache_bucket: String) -> Self {
        Self {
            s3_client,
            cache_bucket,
        }
    }

    /// Create a new S3 storage with default AWS config.
    pub async fn new_with_default_config(cache_bucket: String) -> Self {
        Self::new(
            Client::new(&aws_config::load_from_env().await),
            cache_bucket,
        )
    }
}

impl S3 {
    /// Get the last modified date of the object.
    #[instrument(level = "trace", skip_all)]
    async fn last_modified(
        &self,
        bucket: impl Into<String> + Send,
        key: impl Into<String> + Send,
    ) -> Option<DateTime> {
        self.s3_client
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .ok()
            .and_then(|output| output.last_modified)
    }

    /// Execute a get object request.
    #[instrument(level = "trace", skip_all)]
    async fn get_object<T: for<'de> Deserialize<'de>>(
        &self,
        bucket: impl Into<String> + Send,
        key: impl Into<String> + Send,
    ) -> Result<T> {
        let output = self
            .s3_client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|err| GetObjectError(err.to_string()))?
            .body
            .collect()
            .await
            .map_err(|err| GetObjectError(err.to_string()))?
            .into_bytes();

        from_slice(output.as_ref()).map_err(|err| DeserializeError(err.to_string()))
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
        Ok(self.get_object(bucket, key).await?)
    }
}

#[async_trait]
impl Cache for S3 {
    type Error = Error;
    type Item = Vec<Resolver>;

    #[instrument(level = "trace", skip_all, ret)]
    async fn get<K: AsRef<str> + Send + Sync>(&self, key: K) -> Result<Option<Self::Item>> {
        trace!(key = key.as_ref(), "getting key");

        if let Some(last_modified) = self
            .last_modified(self.cache_bucket.clone(), key.as_ref())
            .await
        {
            let object: CacheItem = self
                .get_object(self.cache_bucket.clone(), key.as_ref())
                .await?;

            if last_modified.as_nanos()
                > DateTime::from(SystemTime::now().sub(Duration::from_secs(object.max_age)))
                    .as_nanos()
            {
                return Ok(Some(object.item));
            }
        }

        Ok(None)
    }

    #[instrument(level = "trace", skip_all, ret)]
    async fn put<K: AsRef<str> + Send + Sync>(
        &self,
        key: K,
        item: Self::Item,
        max_age: u64,
    ) -> Result<()> {
        trace!(key = key.as_ref(), "putting key");

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
            .map_err(|err| {
                let err = err.into_service_error();
                trace!(err = err.message(), "put object error");

                PutObjectError(err.to_string())
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::{from_str, to_string};

    use crate::elsa_endpoint::ElsaManifest;
    use crate::s3::{CacheItem, S3};
    use crate::test_utils::{example_elsa_manifest, with_test_mocks, write_example_manifest};
    use crate::Cache;

    #[tokio::test]
    async fn last_modified() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                write_example_manifest(&manifest_path);

                let result = s3
                    .last_modified("elsa-data-tmp", "htsget-manifests/R004")
                    .await;
                assert!(result.is_some());
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn last_modified_not_found() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                write_example_manifest(&manifest_path);

                let result = s3
                    .last_modified("elsa-data-tmp", "htsget-manifests/R005")
                    .await;
                assert!(result.is_none());
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn get_object() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                write_example_manifest(&manifest_path);

                let result: ElsaManifest = s3
                    .get_object("elsa-data-tmp", "htsget-manifests/R004")
                    .await
                    .unwrap();
                assert_eq!(result, from_str(&example_elsa_manifest()).unwrap());
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn get_object_not_found() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                write_example_manifest(&manifest_path);

                assert!(s3
                    .get_object::<ElsaManifest>("elsa-data-tmp", "htsget-manifests/R005")
                    .await
                    .is_err());
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn get_not_found() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                fs::create_dir_all(&manifest_path).unwrap();
                fs::write(
                    manifest_path.join("R004"),
                    to_string(&CacheItem {
                        item: vec![],
                        max_age: 1000,
                    })
                    .unwrap(),
                )
                .unwrap();

                let result = s3.get("htsget-manifests/R005").await;
                assert!(matches!(result, Ok(None)));
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn get_cache_expired() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                fs::create_dir_all(&manifest_path).unwrap();
                fs::write(
                    manifest_path.join("R004"),
                    to_string(&CacheItem {
                        item: vec![],
                        max_age: 0,
                    })
                    .unwrap(),
                )
                .unwrap();

                let result = s3.get("htsget-manifests/R004").await;
                assert!(matches!(result, Ok(None)));
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn get() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");
                fs::create_dir_all(&manifest_path).unwrap();
                fs::write(
                    manifest_path.join("R004"),
                    to_string(&CacheItem {
                        item: vec![],
                        max_age: 1000,
                    })
                    .unwrap(),
                )
                .unwrap();

                let result = s3.get("htsget-manifests/R004").await.unwrap().unwrap();
                assert!(result.is_empty());
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn put() {
        with_test_mocks(
            |_, s3_client, _, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());

                let manifest_path = base_path.join("elsa-data-tmp");
                fs::create_dir_all(&manifest_path).unwrap();

                s3.put("htsget-manifests/R004", vec![], 1000).await.unwrap();

                let result: CacheItem = from_str(
                    &fs::read_to_string(manifest_path.join("htsget-manifests/R004")).unwrap(),
                )
                .unwrap();

                assert!(result.item.is_empty());
                assert_eq!(result.max_age, 1000);
            },
            0,
        )
        .await;
    }
}
