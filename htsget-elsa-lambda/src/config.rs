use htsget_config::config::parser::from_path;
use htsget_config::config::Config as HtsGetConfig;
use http::uri::Authority;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;
use tracing::instrument;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(flatten)]
    htsget_config: HtsGetConfig,
    #[serde(with = "http_serde::authority")]
    elsa_endpoint: Authority,
    cache_location: String,
}

impl Config {
    #[instrument(level = "debug", ret)]
    pub fn from_path(path: &Path) -> io::Result<Self> {
        from_path(path)
    }

    pub fn htsget_config(&self) -> &HtsGetConfig {
        &self.htsget_config
    }

    pub fn elsa_endpoint(&self) -> &Authority {
        &self.elsa_endpoint
    }

    pub fn cache_location(&self) -> &str {
        &self.cache_location
    }
}
