// src/utils.rs
//use crate::ESQError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub default: DefaultConfig,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct DefaultConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

//Custom Error
#[derive(Debug)]
pub enum ESQError {
    ConfigError(String),
    AuthError,
    NetworkError(String),
    ParseError(String),
    DateParseError(String),
    IOError(std::io::Error),
    ValidationError(String),
    ESError(String),
    NotYetImplemented(String),
}

// Error conversions
impl From<std::io::Error> for ESQError {
    fn from(err: std::io::Error) -> Self {
        ESQError::IOError(err)
    }
}

impl From<reqwest::Error> for ESQError {
    fn from(err: reqwest::Error) -> Self {
        ESQError::NetworkError(err.to_string())
    }
}

impl From<serde_json::Error> for ESQError {
    fn from(err: serde_json::Error) -> Self {
        ESQError::ParseError(err.to_string())
    }
}

impl From<toml::de::Error> for ESQError {
    fn from(err: toml::de::Error) -> Self {
        ESQError::ParseError(format!("TOML deserialization error: {}", err))
    }
}

impl From<toml::ser::Error> for ESQError {
    fn from(err: toml::ser::Error) -> Self {
        ESQError::ParseError(format!("TOML serialization error: {}", err))
    }
}

// Error display
impl std::fmt::Display for ESQError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ESQError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            ESQError::AuthError => write!(f, "Authentication failed"),
            ESQError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ESQError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ESQError::DateParseError(msg) => write!(
                f,
                "Date error: {}\nSee accepted formats at https://docs.rs/dateparser/latest/dateparser/#accepted-date-formats",
                msg
            ),
            ESQError::IOError(err) => write!(f, "IO error: {}", err),
            ESQError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            ESQError::ESError(msg) => write!(f, "Elasticsearch error: {}", msg),
            ESQError::NotYetImplemented(msg) => write!(f, "Not yet implemented: {}", msg),
        }
    }
}

impl std::error::Error for ESQError {}

pub fn load_config(config_file: &PathBuf) -> Result<Option<Config>, ESQError> {
    if config_file.exists() {
        let content = fs::read_to_string(config_file)?;
        let config = toml::from_str(&content)?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

pub fn save_config(config: &Config, config_file: &PathBuf) -> Result<(), ESQError> {
    if let Some(parent_dir) = config_file.parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)?;
            set_dir_permissions(parent_dir)?;
        }
    }

    let toml = toml::to_string(&config)?;
    fs::write(config_file, toml)?;

    let metadata = fs::metadata(config_file)?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(config_file, perms)?;
    Ok(())
}

fn set_dir_permissions(dir: &Path) -> Result<(), ESQError> {
    let metadata = fs::metadata(dir)?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o700);
    fs::set_permissions(dir, perms)?;
    Ok(())
}

pub fn add_auth(
    request: reqwest::blocking::RequestBuilder,
    config: &Config,
) -> reqwest::blocking::RequestBuilder {
    if let (Some(username), Some(password)) = (&config.default.username, &config.default.password) {
        request.basic_auth(username, Some(password))
    } else {
        request
    }
}
