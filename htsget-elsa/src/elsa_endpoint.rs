use crate::Error::{
    DeserializeError, GetManifest, InvalidManifestUri, InvalidReleaseUri,
    UnsupportedManifestFeature,
};
use crate::{Cache, Error, GetObject, ResolversFromElsa, Result};
use async_trait::async_trait;
use htsget_config::resolver::{AllowGuard, Resolver};
use htsget_config::storage::s3::S3Storage;
use htsget_config::storage::Storage;
use htsget_config::types::Format;
use http::uri::{Authority, Parts, Scheme};
use http::Uri;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, instrument};

const ENDPOINT_PATH: &str = "/api/manifest/htsget";
const CACHE_PATH: &str = "htsget-manifest-cache";

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaLocation {
    bucket: String,
    key: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaResponse {
    location: ElsaLocation,
    max_age: u64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaReadsManifest {
    url: String,
    format: Option<Format>,
    restriction: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaVariantsManifest {
    url: String,
    format: Option<Format>,
    variant_sample_id: String,
    restriction: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaRestrictionsManifest {}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaManifest {
    #[serde(alias = "id")]
    release_key: String,
    reads: HashMap<String, ElsaReadsManifest>,
    variants: HashMap<String, ElsaVariantsManifest>,
    restrictions: ElsaRestrictionsManifest,
}

impl ElsaManifest {
    #[instrument(level = "trace", ret)]
    pub fn resolver_from_manifest_parts(
        release_key: &str,
        url: &str,
        id: &str,
        format: Format,
    ) -> Result<Resolver> {
        let uri = Uri::from_str(url)
            .map_err(|err| InvalidManifestUri(err.to_string()))?
            .into_parts();

        match uri.scheme {
            Some(scheme) if scheme.as_str() == "s3" || scheme.as_str() == "S3" => Ok(()),
            _ => Err(UnsupportedManifestFeature(
                "only S3 manifest uris are supported".to_string(),
            )),
        }?;

        let bucket = match uri.authority {
            Some(bucket) => Ok(bucket.to_string()),
            None => Err(InvalidManifestUri("missing bucket from uri".to_string())),
        }?;

        let key = match uri.path_and_query {
            Some(path) => match path.to_string().strip_suffix(format.file_ending()) {
                None => Ok(path.to_string()),
                Some(key) => Ok(key.to_string()),
            },
            None => Err(InvalidManifestUri(
                "missing object path from uri".to_string(),
            )),
        }?;

        Resolver::new(
            Storage::S3 {
                s3_storage: S3Storage::new(bucket),
            },
            &format!("^{}/{}$", release_key, id),
            &key,
            AllowGuard::default().with_allow_formats(vec![format]),
        )
        .map_err(|err| {
            InvalidManifestUri(format!("failed to construct regex: {}", err.to_string()))
        })
    }
}

impl TryFrom<ElsaManifest> for Vec<Resolver> {
    type Error = Error;

    #[instrument(level = "trace", ret)]
    fn try_from(manifest: ElsaManifest) -> Result<Self> {
        let release_key = manifest.release_key;

        manifest
            .reads
            .into_iter()
            .map(|(id, reads_manifest)| {
                ElsaManifest::resolver_from_manifest_parts(
                    &release_key,
                    &reads_manifest.url,
                    &id,
                    reads_manifest.format.unwrap_or(Format::Bam),
                )
            })
            .chain(
                manifest
                    .variants
                    .into_iter()
                    .map(|(id, variants_manifest)| {
                        ElsaManifest::resolver_from_manifest_parts(
                            &release_key,
                            &variants_manifest.url,
                            &id,
                            variants_manifest.format.unwrap_or(Format::Bam),
                        )
                    }),
            )
            .collect()
    }
}

#[derive(Debug)]
pub struct ElsaEndpoint<'a, C, S> {
    endpoint: Authority,
    client: Client,
    cache: &'a C,
    get_object: &'a S,
    scheme: &'a str,
}

#[async_trait]
impl<'a, C, S> ResolversFromElsa for ElsaEndpoint<'a, C, S>
where
    C: Cache<Item = Vec<Resolver>, Error = Error> + Send + Sync,
    S: GetObject<Error = Error> + Send + Sync,
{
    type Error = Error;

    #[instrument(level = "debug", skip_all)]
    async fn try_get(&self, release_key: String) -> Result<Vec<Resolver>> {
        match self.cache.get(&release_key).await {
            Ok(Some(cached)) => Ok(cached),
            _ => {
                debug!("no cached response, fetching from elsa");

                let response = self.get_response(&release_key).await?;
                let max_age = response.max_age;

                let resolvers: Vec<Resolver> = self.get_manifest(response).await?.try_into()?;

                self.cache
                    .put(format!("{CACHE_PATH}/{release_key}"), resolvers.clone(), max_age)
                    .await?;

                Ok(resolvers)
            }
        }
    }
}

impl<'a, C, S> ElsaEndpoint<'a, C, S>
where
    C: Cache<Item = Vec<Resolver>, Error = Error>,
    S: GetObject<Error = Error>,
{
    pub fn new(endpoint: Authority, cache: &'a C, get_object: &'a S) -> Result<Self> {
        Ok(Self::new_with_client(Self::create_client()?, endpoint, cache, get_object, "https"))
    }

    fn new_with_client(client: Client, endpoint: Authority, cache: &'a C, get_object: &'a S, scheme: &'a str) -> Self {
        Self {
            endpoint,
            client,
            cache,
            get_object,
            scheme
        }
    }

    fn create_client() -> Result<Client> {
        Client::builder()
            .use_rustls_tls()
            .https_only(true)
            .build()
            .map_err(|err| Error::InvalidClient(err))
    }

    async fn get_response_with_scheme(&self, release_key: &str, scheme: &str) -> Result<ElsaResponse> {
        let uri = Uri::builder()
            .scheme(scheme)
            .authority(self.endpoint.as_str())
            .path_and_query(format!("{ENDPOINT_PATH}/{release_key}?type=S3"))
            .build()
            .map(|uri| Url::parse(&uri.to_string()))
            .map_err(|_| InvalidReleaseUri(release_key.to_string()))?
            .map_err(|_| InvalidReleaseUri(release_key.to_string()))?;

        let response = self.client
            .get(uri)
            .send()
            .await
            .map_err(|err| GetManifest(err.to_string()))?;

        if response.status().is_success() {
            response.json().await.map_err(|err| DeserializeError(err.to_string()))
        } else {
            Err(GetManifest(response.status().to_string()))
        }
    }

    #[instrument(level = "debug", skip(self), ret)]
    pub async fn get_response(&self, release_key: &str) -> Result<ElsaResponse> {
        self.get_response_with_scheme(release_key, self.scheme).await
    }

    #[instrument(level = "debug", skip(self), ret)]
    pub async fn get_manifest(&self, response: ElsaResponse) -> Result<ElsaManifest> {
        self.get_object
            .get_object(response.location.bucket, response.location.key)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::future::Future;
    use std::str::FromStr;
    use aws_sdk_s3::Client;
    use http::uri::Authority;
    use htsget_test::aws_mocks::with_s3_test_server_tmp;
    use serde_json::from_str;
    use wiremock::{Mock, MockServer, Request, ResponseTemplate, Times};
    use wiremock::matchers::{method, path, query_param};
    use crate::elsa_endpoint::{ElsaEndpoint, ElsaLocation, ElsaResponse, ENDPOINT_PATH};
    use crate::s3::S3;
    use crate::Error::{GetManifest, GetObjectError};
    use crate::ResolversFromElsa;

    #[tokio::test]
    async fn get_response() {
        with_test_mocks(|endpoint, s3_client, reqwest_client| async move {
            let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
            let endpoint = ElsaEndpoint::new_with_client(reqwest_client, Authority::from_str(&endpoint).unwrap(), &s3, &s3, "http");

            let response = endpoint.get_response_with_scheme("R004", "http").await.unwrap();
            assert_eq!(response, ElsaResponse {
                location: ElsaLocation {
                    bucket: "elsa-data-tmp".to_string(),
                    key: "htsget-manifests/R004".to_string(),
                },
                max_age: 86400,
            });
        }, 1).await;
    }

    #[tokio::test]
    async fn get_manifest() {
        with_test_mocks(|endpoint, s3_client, reqwest_client| async move {
            let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
            let endpoint = ElsaEndpoint::new_with_client(reqwest_client, Authority::from_str(&endpoint).unwrap(), &s3, &s3, "http");

            let response = endpoint.get_response_with_scheme("R004", "http").await.unwrap();
            let manifest = endpoint.get_manifest(response).await.unwrap();

            assert_eq!(manifest, from_str(&example_elsa_manifest()).unwrap());
        }, 1).await;
    }

    #[tokio::test]
    async fn get_manifest_not_present() {
        with_test_mocks(|endpoint, s3_client, reqwest_client| async move {
            let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
            let endpoint = ElsaEndpoint::new_with_client(reqwest_client, Authority::from_str(&endpoint).unwrap(), &s3, &s3, "http");

            let manifest = endpoint.get_manifest(from_str(&example_elsa_response()).unwrap()).await;

            assert!(matches!(manifest, Err(GetObjectError(_))));
        }, 0).await;
    }

    fn example_elsa_manifest() -> String {
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
        "#.to_string()
    }

    fn example_elsa_response() -> String {
        r#"
        {
            "location": {
                "bucket": "elsa-data-tmp",
                "key": "htsget-manifests/R004"
            },
            "maxAge": 86400
        }
        "#.to_string()
    }

    async fn with_test_mocks<T, F, Fut>(test: F, expect_times: T)
    where
        T: Into<Times>,
        F: FnOnce(String, Client, reqwest::Client) -> Fut,
          Fut: Future<Output = ()>, {
        with_s3_test_server_tmp(|client, server_base_path| async move {
            let mock_server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path(format!("{ENDPOINT_PATH}/R004")))
                .and(query_param("type", "S3"))
                .respond_with(move |_: &Request| {
                    let manifest_path = server_base_path.join("elsa-data-tmp/htsget-manifests");

                    fs::create_dir_all(&manifest_path).unwrap();
                    fs::write(&manifest_path.join("R004"), example_elsa_manifest()).unwrap();

                    ResponseTemplate::new(200)
                        .set_body_string(example_elsa_response())
                })
                .expect(expect_times)
                .mount(&mock_server)
                .await;

            test(mock_server.address().to_string(), client, reqwest::Client::builder()
                .build().unwrap()).await;
        }).await;
    }
}
