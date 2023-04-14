use crate::Error::{
    DeserializeError, GetGetManifest, InvalidManifestUri, InvalidReleaseUri,
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

const ENDPOINT_PATH: &str = "/manifest/htsget";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaLocation {
    bucket: String,
    key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaResponse {
    location: ElsaLocation,
    max_age: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaReadsManifest {
    url: String,
    format: Option<Format>,
    restriction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaVariantsManifest {
    url: String,
    format: Option<Format>,
    variant_sample_id: String,
    restriction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaRestrictionsManifest {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaManifest {
    #[serde(alias = "id")]
    release_key: String,
    reads: HashMap<String, ElsaReadsManifest>,
    variants: HashMap<String, ElsaVariantsManifest>,
    restrictions: ElsaRestrictionsManifest,
}

impl ElsaManifest {
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
            .chain(manifest.variants.into_iter().map(|(id, variants_manifest)| {
                ElsaManifest::resolver_from_manifest_parts(
                    &release_key,
                    &variants_manifest.url,
                    &id,
                    variants_manifest.format.unwrap_or(Format::Bam),
                )
            }))
            .collect()
    }
}

#[derive(Debug)]
pub struct ElsaEndpoint<C, S> {
    endpoint: Authority,
    client: Client,
    cache: C,
    get_object: S,
}

#[async_trait]
impl<C, S> ResolversFromElsa for ElsaEndpoint<C, S>
where
    C: Cache<Item = Vec<Resolver>, Error = Error> + Send + Sync,
    S: GetObject<Error = Error> + Send + Sync,
{
    type Error = Error;

    async fn try_get(&self, release_key: String) -> Result<Vec<Resolver>> {
        match self.cache.get(&release_key).await {
            Ok(cached) => Ok(cached),
            Err(_) => {
                let response = self.get_response(&release_key).await?;
                let manifest = self.get_manifest(response).await?;

                manifest.try_into()
            }
        }
    }
}

impl<C, S> ElsaEndpoint<C, S>
where
    C: Cache<Item = Vec<Resolver>, Error = Error>,
    S: GetObject<Error = Error>,
{
    pub fn new(endpoint: Authority, cache: C, get_object: S) -> Result<Self> {
        Ok(Self {
            endpoint,
            client: Self::create_client()?,
            cache,
            get_object,
        })
    }

    fn create_client() -> Result<Client> {
        Client::builder()
            .use_rustls_tls()
            .https_only(true)
            .build()
            .map_err(|err| Error::InvalidClient(err))
    }

    pub async fn get_response(&self, release_key: &str) -> Result<ElsaResponse> {
        let uri = http::Uri::builder()
            .scheme("https")
            .authority(self.endpoint.as_str())
            .path_and_query(format!("{}/{}?type=S3", ENDPOINT_PATH, release_key))
            .build()
            .map(|uri| Url::parse(&uri.to_string()))
            .map_err(|_| InvalidReleaseUri(release_key.to_string()))?
            .map_err(|_| InvalidReleaseUri(release_key.to_string()))?;

        self.client
            .get(uri)
            .send()
            .await
            .map_err(|err| GetGetManifest(err))?
            .json()
            .await
            .map_err(|err| DeserializeError(err.to_string()))
    }

    pub async fn get_manifest(&self, response: ElsaResponse) -> Result<ElsaManifest> {
        self.get_object
            .get_object(response.location.bucket, response.location.key)
            .await
    }
}
