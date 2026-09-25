use std::{env, error::Error, fmt, fs, io, path::PathBuf};

use axum::http::{HeaderValue, header::InvalidHeaderValue};
use serde::Deserialize;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

pub const CONFIG_FILE_NAME: &'static str = "config.toml";
pub const CONFIG_VERSION: u32 = 1;

const CONFIG_TEMPLATE: &'static str = include_str!("../res/config.toml");

#[derive(Debug)]
pub enum ConfigError {
    NoParentDirectory,
    Io(io::Error),
    Parse(toml::de::Error),
    UnsupportedVersion(u32),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoParentDirectory => write!(
                f,
                "could not determine the directory to look for {}",
                CONFIG_FILE_NAME
            ),
            Self::Io(error) => write!(f, "could not read or write {}: {}", CONFIG_FILE_NAME, error),
            Self::Parse(error) => write!(f, "could not parse {}: {}", CONFIG_FILE_NAME, error),
            Self::UnsupportedVersion(version) => write!(
                f,
                "unsupported {} version {} (expected {})",
                CONFIG_FILE_NAME, version, CONFIG_VERSION
            ),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Parse(error) => Some(error),
            Self::NoParentDirectory | Self::UnsupportedVersion(_) => None,
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::Parse(value)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub listener: Listener,
}

impl Config {
    pub fn path() -> Result<PathBuf, ConfigError> {
        let exe = env::current_exe()?;
        let dir = exe.parent().ok_or(ConfigError::NoParentDirectory)?;
        Ok(dir.join(CONFIG_FILE_NAME))
    }

    pub fn load_or_create() -> Result<Option<Self>, ConfigError> {
        let path = Self::path()?;
        if !path.exists() {
            fs::write(&path, CONFIG_TEMPLATE)?;
            println!("Created {}", path.display());
            return Ok(None);
        }
        let config: Self = toml::from_str(&fs::read_to_string(&path)?)?;
        if config.version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(config.version));
        }
        Ok(Some(config))
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Listener {
    pub cors: Cors,
    pub serve_docs: bool,
    pub http: HttpListener,
    pub https: HttpsListener,
}

impl Default for Listener {
    fn default() -> Self {
        Self {
            cors: Cors::default(),
            serve_docs: true,
            http: HttpListener::default(),
            https: HttpsListener::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Cors {
    Wildcard(bool),
    Origins(Vec<String>),
}

impl Default for Cors {
    fn default() -> Self {
        Self::Wildcard(true)
    }
}

impl Cors {
    pub fn to_layer(&self) -> Result<CorsLayer, InvalidHeaderValue> {
        let allow_origin = match self {
            Self::Wildcard(true) => AllowOrigin::any(),
            Self::Wildcard(false) => AllowOrigin::list([]),
            Self::Origins(origins) => AllowOrigin::list(
                origins
                    .iter()
                    .map(|origin| HeaderValue::from_str(origin))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        Ok(CorsLayer::new()
            .allow_origin(allow_origin)
            // TODO: 検討の余地あり
            .allow_methods(Any)
            .allow_headers(Any))
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpListener {
    pub enabled: bool,
    pub address: String,
    pub port: u16,
}

impl Default for HttpListener {
    fn default() -> Self {
        Self {
            enabled: true,
            address: "127.0.0.1".to_owned(),
            port: 8080,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HttpsListener {
    pub enabled: bool,
    pub address: String,
    pub port: u16,
    pub cert: String,
    pub key: String,
}

impl Default for HttpsListener {
    fn default() -> Self {
        Self {
            enabled: false,
            address: "127.0.0.1".to_owned(),
            port: 8443,
            cert: String::new(),
            key: String::new(),
        }
    }
}
