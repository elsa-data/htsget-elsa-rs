use std::sync::Arc;

use htsget_config::resolver::Resolver;
use htsget_lambda::RouteType::Id;
use htsget_lambda::{handle_request_service_fn, Route, Router};
use http::{Response, StatusCode};
use lambda_http::{Body, Error, Request};
use tracing::{info, instrument, warn};

use htsget_elsa::elsa_endpoint::ElsaEndpoint;
use htsget_elsa::s3::S3;
use htsget_elsa::{Cache, GetObject, ResolversFromElsa};

use crate::config::Config;

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
                        match ElsaEndpoint::new(config.elsa_endpoint_authority().clone(), &s3, &s3)
                        {
                            Ok(elsa_endpoint) => elsa_endpoint,
                            Err(err) => {
                                return Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Body::from(err.to_string()));
                            }
                        };

                    route_request(&config, event, route, &elsa_endpoint).await
                }
                Err(err) => err,
            }
        },
    )
    .await
}

pub async fn route_request<'a>(
    config: &Config,
    event: Request,
    route: Route,
    elsa_endpoint: &ElsaEndpoint<'a, S3, S3>,
) -> http::Result<Response<Body>> {
    let resolver = get_resolvers(config, &route, elsa_endpoint).await?;
    let router = Router::new(
        Arc::new(resolver),
        config.htsget_config().ticket_server().service_info(),
    );

    router.route_request_with_route(event, route).await
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
        if let Some(release_key) = id.split('/').collect::<Vec<&str>>().first() {
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::str::FromStr;

    use htsget_config::resolver::Resolver;
    use htsget_lambda::Route;
    use htsget_test::http_tests::default_test_config;
    use http::uri::Authority;
    use lambda_http::request::from_str;

    use htsget_elsa::elsa_endpoint::ElsaEndpoint;
    use htsget_elsa::s3::S3;
    use htsget_elsa::test_utils::{is_manifest_resolvers, with_test_mocks};

    use crate::config::Config;
    use crate::get_resolvers;

    #[tokio::test]
    async fn test_route_request() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, _| async move {
                let config = Config::new(
                    default_test_config(),
                    Authority::from_str(&endpoint).unwrap(),
                    "cache".to_string(),
                );

                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    config.elsa_endpoint_authority().clone(),
                    &s3,
                    &s3,
                    "http",
                );

                let response =
                    resolvers_from_endpoint(&config, &endpoint, "data/events/event_get.json").await;

                assert!(!is_manifest_resolvers(response));
            },
            0,
        )
        .await;
    }

    #[tokio::test]
    async fn route_request_elsa_endpoint() {
        with_test_mocks(
            |endpoint, s3_client, reqwest_client, _| async move {
                let config = Config::new(
                    default_test_config(),
                    Authority::from_str(&endpoint).unwrap(),
                    "cache".to_string(),
                );

                let s3 = S3::new(s3_client, "elsa-data-tmp".to_string());
                let endpoint = ElsaEndpoint::new_with_client(
                    reqwest_client,
                    config.elsa_endpoint_authority().clone(),
                    &s3,
                    &s3,
                    "http",
                );

                let response =
                    resolvers_from_endpoint(&config, &endpoint, "data/events/event_elsa.json")
                        .await;

                assert!(is_manifest_resolvers(response));
            },
            1,
        )
        .await;
    }

    async fn resolvers_from_endpoint<'a>(
        config: &Config,
        endpoint: &ElsaEndpoint<'a, S3, S3>,
        path: &str,
    ) -> Vec<Resolver> {
        let path = PathBuf::from_str(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .parent()
            .unwrap()
            .join(path);
        let event = fs::read_to_string(path).unwrap();
        let event = from_str(&event).unwrap();

        let route = Route::try_from(&event).unwrap();

        get_resolvers(config, &route, endpoint).await.unwrap()
    }
}
