use std::io;
use std::path::Path;
use htsget_config::config::Config as HtsGetConfig;
use htsget_config::config::parser::from_path;
use http::uri::Authority;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Config {
    #[serde(flatten)]
    htsget_config: HtsGetConfig,
    root_certificate_id: String,
    identity_id: String,
    parameters_secrets_extension_http_port: u16,
    dynamodb_table_name: String,
    #[serde(with = "http_serde::authority")]
    elsa_endpoint: Authority
}

impl Config {
    pub fn from_path(path: &Path) -> io::Result<Self> {
        from_path(path)
    }
}