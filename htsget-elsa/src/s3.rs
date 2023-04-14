use crate::Error::{DeserializeError, GetObjectError};
use crate::{Error, GetObject, Result};
use async_trait::async_trait;
use aws_sdk_s3::Client;
use serde::Deserialize;
use serde_json::from_slice;

#[derive(Debug)]
pub struct S3 {
    s3_client: Client,
}

#[async_trait]
impl GetObject for S3 {
    type Error = Error;

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
