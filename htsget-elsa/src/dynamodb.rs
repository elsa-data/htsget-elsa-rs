use crate::{Cache, Error, Result};
use async_trait::async_trait;
use aws_sdk_dynamodb::Client;

#[derive(Debug)]
pub struct DynamoDBCache {
    client: Client,
    table_name: String,
}

impl DynamoDBCache {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    pub async fn new_with_default_config(table_name: String) -> Self {
        DynamoDBCache::new(Client::new(&aws_config::load_from_env().await), table_name)
    }
}

#[async_trait]
impl Cache for DynamoDBCache {
    type Error = Error;
    type Item = ();

    async fn get<K: AsRef<str> + Send>(&self, key: K) -> Result<Self::Item> {
        todo!()
    }

    async fn put<K: AsRef<str> + Send>(&self, key: K, item: Self::Item, expiry: u64) {
        todo!()
    }
}
