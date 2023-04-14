pub mod dynamodb;
pub mod elsa_endpoint;

use async_trait::async_trait;
use htsget_config::resolver::Resolver;
use std::str::FromStr;

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
