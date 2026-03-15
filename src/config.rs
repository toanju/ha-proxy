use anyhow::{Context, Result};
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
    /// Address the proxy listens on, e.g. `0.0.0.0:3000`.
    #[serde(default = "default_listen")]
    pub listen: String,
    /// Path to the file containing the HA bearer token. Defaults to `.token`.
    #[serde(default = "default_token_file")]
    pub token_file: String,
    /// Domain / service allow-list.
    #[serde(default)]
    pub allow: Vec<AllowEntry>,
}

fn default_listen() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_token_file() -> String {
    ".token".to_string()
}

impl Config {
    /// Load configuration from `config.toml` in the current directory.
    pub fn load() -> Result<Self> {
        let raw = fs::read_to_string("config.toml").context("failed to read config.toml")?;
        toml::from_str(&raw).context("failed to parse config.toml")
    }
}

/// Read the bearer token from the path specified in `token_file`.
pub fn load_token(path: &str) -> Result<String> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read token file '{}'", path))?;
    let token = raw.trim().to_string();
    anyhow::ensure!(!token.is_empty(), "token file '{}' is empty", path);
    Ok(token)
}
