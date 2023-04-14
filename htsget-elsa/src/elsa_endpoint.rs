use crate::elsa_endpoint::Error::{DeserializeResponseError, GetError, InvalidUri};
use crate::{Cache, ResolverFromElsa};
use async_trait::async_trait;
use htsget_config::resolver::Resolver;
use http::uri::Authority;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::result;
use std::sync::mpsc::SendError;
use thiserror::Error;

const ENDPOINT_PATH: &str = "/manifest/htsget";

pub type Result<T> = result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid client: `{0}`")]
    InvalidClient(reqwest::Error),
    #[error("invalid uri constructed from release key: `{0}`")]
    InvalidUri(String),
    #[error("failed to get uri: `{0}`")]
    GetError(reqwest::Error),
    #[error("failed to deserialize response: `{0}`")]
    DeserializeResponseError(reqwest::Error),
}

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

#[derive(Debug)]
pub struct ElsaEndpoint<C> {
    endpoint: Authority,
    client: Client,
    cache: C,
}

#[async_trait]
impl<C> ResolverFromElsa for ElsaEndpoint<C>
where
    C: Cache + Send + Sync,
{
    async fn get(&self) -> Resolver {
        todo!()
    }
}

impl<C> ElsaEndpoint<C>
where
    C: Cache,
{
    pub fn new(endpoint: Authority, cache: C) -> Result<Self> {
        Ok(Self {
            endpoint,
            client: Self::create_client()?,
            cache,
        })
    }

    fn create_client() -> Result<Client> {
        Client::builder()
            .use_rustls_tls()
            .https_only(true)
            .build()
            .map_err(|err| Error::InvalidClient(err))
    }

    pub async fn get_manifest(&self, release_key: String) -> Result<ElsaResponse> {
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
            .map_err(|err| GetError(err))?
            .json()
            .await
            .map_err(|err| DeserializeResponseError(err))
    }
}
