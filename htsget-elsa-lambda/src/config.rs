use std::io;
use std::path::Path;

use htsget_config::config::parser::from_path;
use htsget_config::config::Config as HtsGetConfig;
use http::uri::Authority;
use serde::{Deserialize, Serialize};

/// Configuration for htsget-elsa. Includes the standard HtsGetConfig.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    #[serde(flatten, default)]
    htsget_config: HtsGetConfig,
    #[serde(with = "http_serde::authority")]
    elsa_endpoint_authority: Authority,
    cache_location: Option<String>,
}

impl Config {
    /// Create a new config.
    pub fn new(
        htsget_config: HtsGetConfig,
        elsa_endpoint_authority: Authority,
        cache_location: Option<String>,
    ) -> Self {
        Self {
            htsget_config,
            elsa_endpoint_authority,
            cache_location,
        }
    }

    /// Get the standard htsget config.
    pub fn htsget_config(&self) -> &HtsGetConfig {
        &self.htsget_config
    }

    /// Get the endpoint authority.
    pub fn elsa_endpoint_authority(&self) -> &Authority {
        &self.elsa_endpoint_authority
    }

    /// Get the cache location.
    pub fn cache_location(&self) -> Option<&str> {
        self.cache_location.as_deref()
    }
}

impl TryFrom<&Path> for Config {
    type Error = io::Error;

    fn try_from(path: &Path) -> io::Result<Self> {
        from_path(path)
    }
}
