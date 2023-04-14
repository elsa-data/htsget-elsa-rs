use crate::Error::{DeserializeError, GetGetManifest, InvalidUri};
use crate::{Cache, Error, GetObject, ResolverFromElsa, Result};
use async_trait::async_trait;
use htsget_config::resolver::Resolver;
use http::uri::Authority;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashMap;

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
    restriction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElsaVariantsManifest {
    url: String,
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

#[derive(Debug)]
pub struct ElsaEndpoint<C, S> {
    endpoint: Authority,
    client: Client,
    cache: C,
    get_object: S,
}

#[async_trait]
impl<C, S> ResolverFromElsa for ElsaEndpoint<C, S>
where
    C: Cache + Send + Sync,
    S: GetObject + Send + Sync,
{
    type Error = Error;

    async fn try_get(&self, release_key: String) -> Result<Resolver> {
        todo!()
    }
}

impl<C, S> ElsaEndpoint<C, S>
where
    C: Cache,
    S: GetObject,
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

    pub async fn get_response(&self, release_key: String) -> Result<ElsaResponse> {
        let uri = http::Uri::builder()
            .scheme("https")
            .authority(self.endpoint.as_str())
            .path_and_query(format!("/manifest/htsget/{}?type=S3", release_key))
            .build()
            .map(|uri| Url::parse(&uri.to_string()))
            .map_err(|_| InvalidUri(release_key.to_string()))?
            .map_err(|_| InvalidUri(release_key.to_string()))?;

        self.client
            .get(uri)
            .send()
            .await
            .map_err(|err| GetGetManifest(err))?
            .json()
            .await
            .map_err(|err| DeserializeError(err.to_string()))
    }

    pub async fn get_manifest(&self, response: ElsaResponse) -> Result<ElsaResponse> {
        todo!()
    }
}
