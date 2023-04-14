use htsget_config::config::cors::CorsConfig;
use htsget_config::config::parser::from_path;
use htsget_config::config::ServiceInfo;
use std::sync::Arc;

use lambda_http::Error;

use htsget_elsa_lambda::handle_request;
use htsget_lambda::Config;

#[tokio::main]
async fn main() -> Result<(), Error> {
    Config::setup_tracing()?;

    if let Some(path) = Config::parse_args() {
        let cors: CorsConfig = from_path(&path)?;
        let service_info: ServiceInfo = from_path(&path)?;

        handle_request(cors, &service_info).await
    } else {
        Ok(())
    }
}