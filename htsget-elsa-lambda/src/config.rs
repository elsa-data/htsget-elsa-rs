use std::io;
use std::path::Path;

use htsget_config::config::parser::from_path;
use htsget_config::config::Config as HtsGetConfig;
use http::uri::Authority;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(flatten, default)]
    htsget_config: HtsGetConfig,
    #[serde(with = "http_serde::authority")]
    elsa_endpoint_authority: Authority,
    cache_location: String,
}

impl Config {
    pub fn new(
        htsget_config: HtsGetConfig,
        elsa_endpoint_authority: Authority,
        cache_location: String,
    ) -> Self {
        Self {
            htsget_config,
            elsa_endpoint_authority,
            cache_location,
        }
    }

    pub fn htsget_config(&self) -> &HtsGetConfig {
        &self.htsget_config
    }

    pub fn elsa_endpoint_authority(&self) -> &Authority {
        &self.elsa_endpoint_authority
    }

    pub fn cache_location(&self) -> &str {
        &self.cache_location
    }

    #[instrument(level = "debug", ret)]
    pub fn from_path(path: &Path) -> io::Result<Self> {
        from_path(path)
    }
}
