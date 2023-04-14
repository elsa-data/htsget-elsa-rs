pub mod elsa_endpoint;
pub mod dynamodb;

use std::str::FromStr;
use async_trait::async_trait;
use htsget_config::resolver::Resolver;

#[async_trait]
pub trait Cache {
    type Item;

    async fn get<K: AsRef<str> + Send>(&self, key: K) -> Self::Item;
    async fn put<K: AsRef<str> + Send>(&self, key: K, item: Self::Item);
}

#[async_trait]
pub trait ResolverFromElsa {
    async fn get(&self) -> Resolver;
}