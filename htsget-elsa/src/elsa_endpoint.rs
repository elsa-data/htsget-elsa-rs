use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use htsget_config::resolver::{AllowGuard, ReferenceNames, Resolver};
use htsget_config::storage::s3::S3Storage;
use htsget_config::storage::Storage;
use htsget_config::types::{Format, Interval};
use http::uri::Authority;
use http::Uri;
use reqwest::{Client, Url};
use serde::Deserialize;
use tracing::{debug, instrument};

use crate::Error::{
    DeserializeError, GetManifest, InvalidManifest, InvalidReleaseUri, UnsupportedManifestFeature,
};
use crate::{Cache, Error, GetObject, ResolversFromElsa, Result};

pub const ENDPOINT_PATH: &str = "/api/manifest/htsget";
pub const CACHE_PATH: &str = "htsget-manifest-cache";

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
    restrictions: Vec<ElsaRestrictionManifest>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaVariantsManifest {
    url: String,
    format: Option<Format>,
    variant_sample_id: String,
    restrictions: Vec<ElsaRestrictionManifest>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaRestrictionManifest {
    chromosome: u8,
    start: Option<u32>,
    end: Option<u32>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ElsaManifest {
    #[serde(alias = "id")]
    release_key: String,
    reads: HashMap<String, ElsaReadsManifest>,
    variants: HashMap<String, ElsaVariantsManifest>,
}

impl ElsaManifest {
    #[instrument(level = "trace", ret)]
    pub fn resolver_from_manifest_parts(
        release_key: &str,
        url: &str,
        id: &str,
        format: Format,
        restriction: &ElsaRestrictionManifest,
    ) -> Result<Resolver> {
        let url = match (url.strip_prefix("s3://"), url.strip_prefix("S3://")) {
            (Some(url), _) | (_, Some(url)) => Ok(url),
            _ => Err(UnsupportedManifestFeature(
                "only S3 manifest uris are supported".to_string(),
            )),
        }?;

        let (bucket, key) = match url.split_once('/') {
            Some(split) => Ok(split),
            None => Err(InvalidManifest(
                "could not split url into bucket and object key".to_string(),
            )),
        }?;

        if bucket.is_empty() || key.is_empty() {
            return Err(InvalidManifest("bucket or key is empty".to_string()));
        }

        let key = match key.to_string().strip_suffix(format.file_ending()) {
            None => key.to_string(),
            Some(key) => key.to_string(),
        };

        Resolver::new(
            Storage::S3 {
                s3_storage: S3Storage::new(bucket.to_string()),
            },
            &format!("^{}/{}$", release_key, id),
            &key,
            AllowGuard::default()
                .with_allow_formats(vec![format])
                .with_allow_reference_names(ReferenceNames::List(HashSet::from_iter(vec![
                    restriction.chromosome.to_string(),
                ])))
                .with_allow_interval(Interval::new(restriction.start, restriction.end)),
        )
        .map_err(|err| InvalidManifest(format!("failed to construct regex: {}", err)))
    }
}

impl TryFrom<ElsaManifest> for Vec<Resolver> {
    type Error = Error;

    #[instrument(level = "trace", ret)]
    fn try_from(manifest: ElsaManifest) -> Result<Self> {
        let release_key = manifest.release_key;

        let get_resolvers =
            |url: &str, id: &str, format: Format, restrictions: Vec<ElsaRestrictionManifest>| {
                restrictions
                    .iter()
                    .map(|restriction| {
                        ElsaManifest::resolver_from_manifest_parts(
                            &release_key,
                            url,
                            id,
                            format,
                            restriction,
                        )
                    })
                    .collect()
            };

        Ok(manifest
            .reads
            .into_iter()
            .map(|(id, reads_manifest)| {
                get_resolvers(
                    &reads_manifest.url,
                    &id,
                    reads_manifest.format.unwrap_or(Format::Bam),
                    reads_manifest.restrictions,
                )
            })
            .chain(
                manifest
                    .variants
                    .into_iter()
                    .map(|(id, variants_manifest)| {
                        get_resolvers(
                            &variants_manifest.url,
                            &id,
                            variants_manifest.format.unwrap_or(Format::Vcf),
                            variants_manifest.restrictions,
                        )
                    }),
            )
            .collect::<Result<Vec<Vec<Resolver>>>>()?
            .into_iter()
            .flatten()
            .collect())
    }
}

/// Implements the mechanism which fetches manifests from Elsa.
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
        let cache_key = format!("{CACHE_PATH}/{release_key}");

        match self.cache.get(&cache_key).await {
            Ok(Some(cached)) => Ok(cached),
            _ => {
                debug!("no cached response, fetching from elsa");

                let response = self.get_response(&release_key).await?;
                let max_age = response.max_age;

                let resolvers: Vec<Resolver> = self.get_manifest(response).await?.try_into()?;

                self.cache
                    .put(cache_key, resolvers.clone(), max_age)
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
        Ok(Self {
            client: Self::create_client()?,
            endpoint,
            cache,
            get_object,
            scheme: "https",
        })
    }

    #[cfg(feature = "test-utils")]
    pub fn new_with_client(
        client: Client,
        endpoint: Authority,
        cache: &'a C,
        get_object: &'a S,
        scheme: &'a str,
    ) -> Self {
        Self {
            endpoint,
            client,
            cache,
            get_object,
            scheme,
        }
    }

    fn create_client() -> Result<Client> {
        Client::builder()
            .use_rustls_tls()
            .https_only(true)
            .build()
            .map_err(Error::InvalidClient)
    }

    async fn get_response_with_scheme(
        &self,
        release_key: &str,
        scheme: &str,
    ) -> Result<ElsaResponse> {
        let uri = Uri::builder()
            .scheme(scheme)
            .authority(self.endpoint.as_str())
            .path_and_query(format!("{ENDPOINT_PATH}/{release_key}?type=S3"))
            .build()
            .map(|uri| Url::parse(&uri.to_string()))
            .map_err(|_| InvalidReleaseUri(release_key.to_string()))?
            .map_err(|_| InvalidReleaseUri(release_key.to_string()))?;

        let response = self
            .client
            .get(uri)
            .send()
            .await
            .map_err(|err| GetManifest(err.to_string()))?;

        if response.status().is_success() {
            response
                .json()
                .await
                .map_err(|err| DeserializeError(err.to_string()))
        } else {
            Err(GetManifest(response.status().to_string()))
        }
    }

    #[instrument(level = "debug", skip(self), ret)]
    pub async fn get_response(&self, release_key: &str) -> Result<ElsaResponse> {
        self.get_response_with_scheme(release_key, self.scheme)
            .await
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
    use std::str::FromStr;

    use htsget_config::resolver::Resolver;
    use htsget_config::types::Format;
    use http::uri::Authority;
    use serde_json::from_str;

    use crate::elsa_endpoint::{
        ElsaEndpoint, ElsaLocation, ElsaManifest, ElsaResponse, ElsaRestrictionManifest, CACHE_PATH,
    };
    use crate::s3::S3;
    use crate::test_utils::{
        example_elsa_manifest, example_elsa_response, is_manifest_resolvers,
        is_resolver_from_parts, with_test_mocks,
    };
    use crate::Error::{GetObjectError, InvalidManifest, UnsupportedManifestFeature};
    use crate::{Cache, ResolversFromElsa};

    #[tokio::test]
    async fn get_response() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, _| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    Authority::from_str(&endpoint).unwrap(),
                    &s3,
                    &s3,
                    "http",
                );

                let response = endpoint.get_response("R004").await.unwrap();
                assert_eq!(
                    response,
                    ElsaResponse {
                        location: ElsaLocation {
                            bucket: "elsa-data-tmp".to_string(),
                            key: "htsget-manifests/R004".to_string(),
                        },
                        max_age: 86400,
                    }
                );
            },
            1,
        )
        .await;
    }

    #[tokio::test]
    async fn get_manifest() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, _| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    Authority::from_str(&endpoint).unwrap(),
                    &s3,
                    &s3,
                    "http",
                );

                let response = endpoint.get_response("R004").await.unwrap();
                let manifest = endpoint.get_manifest(response).await.unwrap();

                assert_eq!(manifest, from_str(&example_elsa_manifest()).unwrap());
            },
            1,
        )
        .await;
    }

    #[tokio::test]
    async fn get_manifest_not_present() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, _| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    Authority::from_str(&endpoint).unwrap(),
                    &s3,
                    &s3,
                    "http",
                );

                let manifest = endpoint
                    .get_manifest(from_str(&example_elsa_response()).unwrap())
                    .await;

                assert!(matches!(manifest, Err(GetObjectError(_))));
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn try_get_cached() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, _| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    Authority::from_str(&endpoint).unwrap(),
                    &s3,
                    &s3,
                    "http",
                );

                s3.put(format!("{CACHE_PATH}/R004"), vec![], 1000)
                    .await
                    .unwrap();

                let resolvers = endpoint.try_get("R004".to_string()).await.unwrap();
                assert!(resolvers.is_empty());
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn try_get_not_cached() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, base_path| async move {
                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    Authority::from_str(&endpoint).unwrap(),
                    &s3,
                    &s3,
                    "http",
                );

                assert!(!base_path
                    .join(format!("elsa-data-tmp/{CACHE_PATH}/R004"))
                    .exists());
                let resolvers = endpoint.try_get("R004".to_string()).await.unwrap();

                assert!(is_manifest_resolvers(resolvers));
                assert!(base_path
                    .join(format!("elsa-data-tmp/{CACHE_PATH}/R004"))
                    .exists());
            },
            1,
        )
        .await;
    }

    #[test]
    fn resolvers_from_manifest() {
        let manifest: ElsaManifest = from_str(&example_elsa_manifest()).unwrap();
        let resolvers: Vec<Resolver> = manifest.try_into().unwrap();
        assert!(is_manifest_resolvers(resolvers));
    }

    #[test]
    fn resolver_from_parts() {
        let response = ElsaManifest::resolver_from_manifest_parts(
            "R004",
            "s3://umccr-10g-data-dev/HG00097/HG00097.bam",
            "30F9F3FED8F711ED8C35DBEF59E9F537",
            Format::Bam,
            &example_restrictions_manifest(),
        )
        .unwrap();
        assert!(is_resolver_from_parts(&response));
    }

    #[test]
    fn resolver_from_parts_uppercase() {
        let response = ElsaManifest::resolver_from_manifest_parts(
            "R004",
            "S3://umccr-10g-data-dev/HG00097/HG00097.bam",
            "30F9F3FED8F711ED8C35DBEF59E9F537",
            Format::Bam,
            &example_restrictions_manifest(),
        )
        .unwrap();
        assert!(is_resolver_from_parts(&response));
    }

    #[test]
    fn resolver_from_parts_no_file_ending() {
        let response = ElsaManifest::resolver_from_manifest_parts(
            "R004",
            "s3://umccr-10g-data-dev/HG00097/HG00097",
            "30F9F3FED8F711ED8C35DBEF59E9F537",
            Format::Bam,
            &example_restrictions_manifest(),
        )
        .unwrap();
        assert!(is_resolver_from_parts(&response));
    }

    #[test]
    fn resolver_from_parts_invalid_scheme() {
        let response = ElsaManifest::resolver_from_manifest_parts(
            "R004",
            "gcp://umccr-10g-data-dev/HG00097/HG00097.bam",
            "30F9F3FED8F711ED8C35DBEF59E9F537",
            Format::Bam,
            &example_restrictions_manifest(),
        );
        assert!(matches!(response, Err(UnsupportedManifestFeature(_))));
    }

    #[test]
    fn resolver_from_parts_no_object_key() {
        let response = ElsaManifest::resolver_from_manifest_parts(
            "R004",
            "s3://umccr-10g-data-dev",
            "30F9F3FED8F711ED8C35DBEF59E9F537",
            Format::Bam,
            &example_restrictions_manifest(),
        );
        assert!(matches!(response, Err(InvalidManifest(_))));
    }

    #[test]
    fn resolver_from_parts_no_bucket() {
        let response = ElsaManifest::resolver_from_manifest_parts(
            "R004",
            "s3:///HG00097/HG00097.bam",
            "30F9F3FED8F711ED8C35DBEF59E9F537",
            Format::Bam,
            &example_restrictions_manifest(),
        );
        assert!(matches!(response, Err(InvalidManifest(_))));
    }

    fn example_restrictions_manifest() -> ElsaRestrictionManifest {
        ElsaRestrictionManifest {
            chromosome: 1,
            start: Some(1),
            end: Some(10),
        }
    }
}
