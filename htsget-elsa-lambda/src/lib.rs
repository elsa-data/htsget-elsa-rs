use std::sync::Arc;
use htsget_config::config::cors::CorsConfig;
use htsget_config::config::ServiceInfo;
use htsget_config::resolver::Resolver;
use htsget_lambda::{handle_request_service_fn, Router};
use lambda_http::{Error, Request};
use tracing::info;

pub mod config;

pub async fn handle_request(cors: CorsConfig, service_info: &ServiceInfo) -> Result<(), Error>
{
    handle_request_service_fn(cors, |event: Request| async move {
        info!(event = ?event, "received request");

        let resolver = get_resolvers(&event).await;
        let router = Router::new(Arc::new(resolver), &service_info);

        router.route_request(event).await
    })
        .await
}

pub async fn get_resolvers(event: &Request) -> Vec<Resolver> {
    todo!();
}