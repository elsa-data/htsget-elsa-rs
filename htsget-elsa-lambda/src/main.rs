use htsget_config::config::parser::from_path;
use htsget_lambda::Config as HtsgetConfig;
use lambda_http::Error;

use htsget_elsa_lambda::config::Config;
use htsget_elsa_lambda::handle_request;

#[tokio::main]
async fn main() -> Result<(), Error> {
    if let Some(path) = HtsgetConfig::parse_args() {
        let config: Config = from_path(&path)?;

        config.htsget_config().setup_tracing()?;

        handle_request(config).await
    } else {
        Ok(())
    }
}
