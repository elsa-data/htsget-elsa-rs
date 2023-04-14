use crate::config::Config;
use htsget_config::config::cors::CorsConfig;
use htsget_config::config::ServiceInfo;
use htsget_config::resolver::Resolver;
use htsget_elsa::elsa_endpoint::ElsaEndpoint;
use htsget_elsa::s3::S3;
use htsget_elsa::{Cache, GetObject, ResolversFromElsa};
use htsget_lambda::handlers::FormatJson;
use htsget_lambda::RouteType::Id;
use htsget_lambda::{handle_request_service_fn, Route, Router};
use http::{Response, StatusCode};
use lambda_http::{Body, Error, Request};
use std::sync::Arc;
use tracing::{info, instrument, warn};

pub mod config;

pub async fn handle_request(config: Config) -> Result<(), Error> {
    handle_request_service_fn(
        config.htsget_config().ticket_server().cors().clone(),
        |event: Request| async {
            info!(event = ?event, "received request");

            match Route::try_from(&event) {
                Ok(route) => {
                    let s3 = S3::new_with_default_config(config.cache_location().to_string()).await;
                    let elsa_endpoint =
                        match ElsaEndpoint::new(config.elsa_endpoint().clone(), &s3, &s3) {
                            Ok(elsa_endpoint) => elsa_endpoint,
                            Err(err) => {
                                return Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Body::from(err.to_string()));
                            }
                        };

                    let resolver = get_resolvers(&config, &route, &elsa_endpoint).await?;
                    let router = Router::new(
                        Arc::new(resolver),
                        &config.htsget_config().ticket_server().service_info(),
                    );
                    router.route_request_with_route(event, route).await
                }
                Err(err) => err,
            }
        },
    )
    .await
}

#[instrument(level = "debug", skip(elsa_endpoint), ret)]
pub async fn get_resolvers<'a, C, S>(
    config: &Config,
    route: &Route,
    elsa_endpoint: &'a ElsaEndpoint<'a, C, S>,
) -> http::Result<Vec<Resolver>>
where
    C: Cache<Item = Vec<Resolver>, Error = htsget_elsa::Error> + Send + Sync,
    S: GetObject<Error = htsget_elsa::Error> + Send + Sync,
{
    if let Id(id) = route.route_type() {
        if let Some(release_key) = id.split("/").collect::<Vec<&str>>().first() {
            if let Ok(mut resolvers) = elsa_endpoint.try_get(release_key.to_string()).await {
                resolvers.append(&mut config.htsget_config().resolvers().to_vec());

                return Ok(resolvers);
            }
        }
    }

    warn!(
        "failed to get resolvers from elsa endpoint, attempting to use only resolvers from config"
    );

    Ok(config.htsget_config().resolvers().to_vec())
}
