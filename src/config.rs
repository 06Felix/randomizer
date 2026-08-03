use std::{
    env,
    net::{IpAddr, Ipv4Addr},
};

use thiserror::Error;

use crate::state::AppState;

const DEFAULT_PORT: u16 = 7263;
const DEFAULT_LOG_FILTER: &str = "info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub max_concurrent_ws_streams: usize,
    pub log_filter: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: DEFAULT_PORT,
            max_concurrent_ws_streams: AppState::DEFAULT_MAX_CONCURRENT_WS_STREAMS,
            log_filter: DEFAULT_LOG_FILTER.to_string(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let host = parse_or_default(
            &mut lookup,
            "RANDOMIZER_HOST",
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        )?;
        let port = parse_or_default(&mut lookup, "RANDOMIZER_PORT", DEFAULT_PORT)?;
        let max_concurrent_ws_streams = parse_or_default(
            &mut lookup,
            "RANDOMIZER_MAX_CONCURRENT_WS_STREAMS",
            AppState::DEFAULT_MAX_CONCURRENT_WS_STREAMS,
        )?;
        if max_concurrent_ws_streams == 0 {
            return Err(ConfigError::ZeroValue {
                name: "RANDOMIZER_MAX_CONCURRENT_WS_STREAMS",
            });
        }

        let log_filter = lookup("RUST_LOG").unwrap_or_else(|| DEFAULT_LOG_FILTER.to_string());
        tracing_subscriber::EnvFilter::try_new(&log_filter).map_err(|source| {
            ConfigError::InvalidLogFilter {
                value: log_filter.clone(),
                source,
            }
        })?;

        Ok(Self {
            host,
            port,
            max_concurrent_ws_streams,
            log_filter,
        })
    }
}

fn parse_or_default<T>(
    lookup: &mut impl FnMut(&str) -> Option<String>,
    name: &'static str,
    default: T,
) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let Some(value) = lookup(name) else {
        return Ok(default);
    };
    value.parse().map_err(|source| ConfigError::InvalidValue {
        name,
        value,
        source: Box::new(source),
    })
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid value {value:?} for {name}: {source}")]
    InvalidValue {
        name: &'static str,
        value: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("{name} must be greater than zero")]
    ZeroValue { name: &'static str },
    #[error("invalid RUST_LOG filter {value:?}: {source}")]
    InvalidLogFilter {
        value: String,
        #[source]
        source: tracing_subscriber::filter::ParseError,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn uses_defaults_when_environment_is_empty() {
        let config = Config::from_lookup(|_| None).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn loads_all_supported_settings() {
        let values = HashMap::from([
            ("RANDOMIZER_HOST", "127.0.0.1"),
            ("RANDOMIZER_PORT", "8080"),
            ("RANDOMIZER_MAX_CONCURRENT_WS_STREAMS", "12"),
            ("RUST_LOG", "randomizer=debug"),
        ]);
        let config = Config::from_lookup(|name| values.get(name).map(ToString::to_string)).unwrap();

        assert_eq!(config.host, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(config.port, 8080);
        assert_eq!(config.max_concurrent_ws_streams, 12);
        assert_eq!(config.log_filter, "randomizer=debug");
    }

    #[test]
    fn rejects_zero_websocket_limit() {
        let error = Config::from_lookup(|name| {
            (name == "RANDOMIZER_MAX_CONCURRENT_WS_STREAMS").then(|| "0".to_string())
        })
        .unwrap_err();

        assert!(matches!(error, ConfigError::ZeroValue { .. }));
    }
}
