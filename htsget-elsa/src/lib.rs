use std::result;

use async_trait::async_trait;
use htsget_config::resolver::Resolver;
use serde::Deserialize;
use thiserror::Error;

pub mod elsa_endpoint;
pub mod s3;

pub type Result<T> = result::Result<T, Error>;

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

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;
    use std::future::Future;
    use std::path::{Path, PathBuf};

    use crate::elsa_endpoint::ENDPOINT_PATH;
    use aws_sdk_s3::Client;
    use htsget_test::aws_mocks::with_s3_test_server_tmp;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate, Times};

    pub(crate) fn example_elsa_manifest() -> String {
        r#"
        {
            "id": "R004",
            "reads": {
                "30F9F3FED8F711ED8C35DBEF59E9F537": {
                    "url": "s3://umccr-10g-data-dev/HG00097/HG00097.bam"
                },
                "30F9FFD4D8F711ED8C353BBCB8861211": {
                    "url": "s3://umccr-10g-data-dev/HG00096/HG00096.bam"
                }
            },
            "variants": {
                "30F9F3FED8F711ED8C35DBEF59E9F537": {
                    "url": "s3://umccr-10g-data-dev/HG00097/HG00097.hard-filtered.vcf.gz",
                    "variantSampleId": ""
                },
                "30F9FFD4D8F711ED8C353BBCB8861211": {
                    "url": "s3://umccr-10g-data-dev/HG00096/HG00096.hard-filtered.vcf.gz",
                    "variantSampleId": ""
                }
            },
            "restrictions": {},
            "cases": [
                {
                    "ids": { "": "SINGLETONCHARLES" },
                    "patients": [
                        {
                            "ids": { "": "CHARLES" },
                            "specimens": [
                                {
                                    "htsgetId": "30F9FFD4D8F711ED8C353BBCB8861211",
                                    "ids": { "": "HG00096" }
                                }
                            ]
                        }
                    ]
                },
                {
                    "ids": { "": "SINGLETONMARY" },
                    "patients": [
                        {
                            "ids": { "": "MARY" },
                            "specimens": [
                                {
                                    "htsgetId": "30F9F3FED8F711ED8C35DBEF59E9F537",
                                    "ids": { "": "HG00097" }
                                }
                            ]
                        }
                    ]
                }
            ]
        }
        "#
        .to_string()
    }

    pub(crate) fn example_elsa_response() -> String {
        r#"
        {
            "location": {
                "bucket": "elsa-data-tmp",
                "key": "htsget-manifests/R004"
            },
            "maxAge": 86400
        }
        "#
        .to_string()
    }

    pub(crate) fn write_example_manifest(manifest_path: &Path) {
        fs::create_dir_all(manifest_path).unwrap();
        fs::write(manifest_path.join("R004"), example_elsa_manifest()).unwrap();
    }

    pub(crate) async fn with_test_mocks<T, F, Fut>(test: F, expect_times: T)
    where
        T: Into<Times>,
        F: FnOnce(String, Client, reqwest::Client, PathBuf) -> Fut,
        Fut: Future<Output = ()>,
    {
        with_s3_test_server_tmp(|client, server_base_path| async move {
            let mock_server = MockServer::start().await;

            let base_path = server_base_path.clone();
            Mock::given(method("GET"))
                .and(path(format!("{ENDPOINT_PATH}/R004")))
                .and(query_param("type", "S3"))
                .respond_with(move |_: &Request| {
                    let manifest_path = base_path.join("elsa-data-tmp/htsget-manifests");

                    write_example_manifest(&manifest_path);

                    ResponseTemplate::new(200).set_body_string(example_elsa_response())
                })
                .expect(expect_times)
                .mount(&mock_server)
                .await;

            test(
                mock_server.address().to_string(),
                client,
                reqwest::Client::builder().build().unwrap(),
                server_base_path,
            )
            .await;
        })
        .await;
    }
}
