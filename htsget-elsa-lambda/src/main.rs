use htsget_config::config::cors::CorsConfig;
use htsget_config::config::parser::from_path;
use htsget_config::config::ServiceInfo;
use std::sync::Arc;

use lambda_http::Error;

use htsget_elsa_lambda::config::Config;
use htsget_elsa_lambda::handle_request;
use htsget_lambda::Config as HtsgetConfig;

#[tokio::main]
async fn main() -> Result<(), Error> {
    HtsgetConfig::setup_tracing()?;

    if let Some(path) = HtsgetConfig::parse_args() {
        let config: Config = from_path(&path)?;

        handle_request(config).await
    } else {
        Ok(())
    }
}
