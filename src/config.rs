use anyhow::{Context, Result};
use reqwest::Url;
use secrecy::SecretString;
use serde::Deserialize;
use std::fs;

/// A single entry in the [[allow]] array.
#[derive(Debug, Clone, Deserialize)]
pub struct AllowEntry {
    pub domain: String,
    /// Empty list means all services for this domain are permitted.
    #[serde(default)]
    pub services: Vec<String>,
}

/// Top-level configuration loaded from `config.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Base URL of the Home Assistant instance, e.g. `http://homeassistant.local:8123`.
    pub ha_url: String,
    /// Address the proxy listens on, e.g. `0.0.0.0:8080`.
    #[serde(default = "default_listen")]
    pub listen: String,
    /// Path to the file containing the HA bearer token. Defaults to `.token`.
    #[serde(default = "default_token_file")]
    pub token_file: String,
    /// Maximum allowed incoming request body size in bytes. Defaults to 65536 (64 KiB).
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: usize,
    /// Domain / service allow-list.
    #[serde(default)]
    pub allow: Vec<AllowEntry>,
}

fn default_listen() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_token_file() -> String {
    ".token".to_string()
}

fn default_max_body_bytes() -> usize {
    65536
}

impl Config {
    /// Load and validate configuration from the given path.
    pub fn load(path: &str) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file '{}'", path))?;
        let cfg: Self = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config file '{}'", path))?;
        cfg.validate()
    }

    fn validate(self) -> Result<Self> {
        // Parse and validate ha_url: must be a well-formed http/https URL.
        let url = Url::parse(&self.ha_url)
            .with_context(|| format!("invalid ha_url '{}': not a valid URL", self.ha_url))?;

        anyhow::ensure!(
            url.scheme() == "http" || url.scheme() == "https",
            "invalid ha_url '{}': scheme must be http or https, got '{}'",
            self.ha_url,
            url.scheme()
        );

        // Normalise: strip trailing slash so proxy.rs can unconditionally append the path.
        let ha_url = self.ha_url.trim_end_matches('/').to_string();

        Ok(Self { ha_url, ..self })
    }
}

/// Read the bearer token from the path specified in `token_file`.
///
/// Returns a [`SecretString`] so the value is zeroed on drop and cannot
/// be accidentally printed via `Debug` or `Display`.
pub fn load_token(path: &str) -> Result<SecretString> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read token file '{}'", path))?;
    let token = raw.trim().to_string();
    anyhow::ensure!(!token.is_empty(), "token file '{}' is empty", path);
    Ok(SecretString::from(token))
}
