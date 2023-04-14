use std::result;
use htsget_config::resolver::Resolver;
use http::uri::Authority;
use crate::{Cache, ResolverFromElsa};
use async_trait::async_trait;
use reqwest::{Certificate, Client, ClientBuilder, Identity};
use thiserror::Error;

pub type Result<T> = result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid certificate: `{0}`")]
    InvalidCertificate(reqwest::Error),
    #[error("invalid identity: `{0}`")]
    InvalidIdentity(reqwest::Error),
    #[error("invalid client: `{0}`")]
    InvalidClient(reqwest::Error),
}

#[derive(Debug)]
pub struct ElsaEndpoint<C> {
    endpoint: Authority,
    client: Client,
    cache: C
}

#[async_trait]
impl<C> ResolverFromElsa for ElsaEndpoint<C>
where C: Cache + Send + Sync {
    async fn get(&self) -> Resolver {
        todo!()
    }
}

impl<C> ElsaEndpoint<C>
    where C: Cache {
    pub fn new(endpoint: Authority, cache: C, root_certificate: String, identity: String) -> Result<Self> {
        Ok(Self { endpoint, client: Self::create_mtls_client(root_certificate, identity)?, cache })
    }

    fn create_mtls_client(root_certificate: String, identity: String) -> Result<Client> {
        Client::builder()
            .use_rustls_tls()
            .tls_built_in_root_certs(false)
            .add_root_certificate(Self::create_cert(root_certificate)?)
            .identity(Self::create_identity(identity)?)
            .https_only(true)
            .build()
            .map_err(|err| Error::InvalidClient(err))
    }

    fn create_cert(certificate: String) -> Result<Certificate> {
        Certificate::from_pem(&certificate.into_bytes()).map_err(|err| Error::InvalidCertificate(err))
    }

    fn create_identity(certificate: String) -> Result<Identity> {
        Identity::from_pem(&certificate.into_bytes()).map_err(|err| Error::InvalidIdentity(err))
    }

    pub fn get_manifest(&self) -> String {
       todo!()
    }
}