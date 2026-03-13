use std::{env, net::SocketAddr, str::FromStr};

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub watcher: WatcherConfig,
    pub convex: ConvexClientConfig,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub frontend_url: String,
}

#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub base_rpc_url: String,
    pub base_wss_url: String,
    pub chain_id: u64,
    pub network: String,
    pub required_confirmations: u64,
    pub polling_interval_secs: u64,
    pub start_block: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ConvexClientConfig {
    pub deployment_url: String,
    pub site_url: Option<String>,
    pub admin_key: Option<String>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    MissingVar(&'static str),

    #[error("invalid environment variable `{name}`: {message}")]
    InvalidVar { name: &'static str, message: String },
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerConfig::from_env()?,
            watcher: WatcherConfig::from_env()?,
            convex: ConvexClientConfig::from_env()?,
        })
    }
}

impl ServerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let port = parse_env_or_default::<u16>("PORT", 8080)?;
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let bind_addr = SocketAddr::from_str(&format!("{host}:{port}")).map_err(|e| {
            ConfigError::InvalidVar {
                name: "HOST/PORT",
                message: format!("failed to construct socket address: {e}"),
            }
        })?;

        let frontend_url =
            env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

        Ok(Self {
            bind_addr,
            frontend_url,
        })
    }
}

impl WatcherConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            base_rpc_url: required_env("BASE_RPC_URL")?,
            base_wss_url: required_env("BASE_WSS_URL")?,
            chain_id: parse_env_or_default("BASE_CHAIN_ID", 8453)?,
            network: env::var("BASE_NETWORK").unwrap_or_else(|_| "base".to_string()),
            required_confirmations: parse_env_or_default("REQUIRED_CONFIRMATIONS", 6)?,
            polling_interval_secs: parse_env_or_default("WATCHER_POLL_INTERVAL_SECS", 10)?,
            start_block: optional_parse_env("WATCHER_START_BLOCK")?,
        })
    }
}

impl ConvexClientConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            deployment_url: required_env("CONVEX_URL")?,
            site_url: optional_env("CONVEX_SITE_URL"),
            admin_key: optional_env("CONVEX_ADMIN_KEY"),
        })
    }
}

fn required_env(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::MissingVar(name))
}

fn optional_env(name: &'static str) -> Option<String> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => None,
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

fn parse_env_or_default<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) => value.parse::<T>().map_err(|e| ConfigError::InvalidVar {
            name,
            message: e.to_string(),
        }),
        Err(_) => Ok(default),
    }
}

fn optional_parse_env<T>(name: &'static str) -> Result<Option<T>, ConfigError>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|e| ConfigError::InvalidVar {
                name,
                message: e.to_string(),
            }),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_env() {
        for key in [
            "HOST",
            "PORT",
            "FRONTEND_URL",
            "BASE_RPC_URL",
            "BASE_WSS_URL",
            "BASE_CHAIN_ID",
            "BASE_NETWORK",
            "REQUIRED_CONFIRMATIONS",
            "WATCHER_POLL_INTERVAL_SECS",
            "WATCHER_START_BLOCK",
            "CONVEX_URL",
            "CONVEX_SITE_URL",
            "CONVEX_ADMIN_KEY",
        ] {
            env::remove_var(key);
        }
    }

    #[test]
    fn parses_defaults_for_optional_values() {
        let _guard = env_lock().lock().unwrap();
        clear_env();

        env::set_var("BASE_RPC_URL", "https://mainnet.base.org");
        env::set_var("BASE_WSS_URL", "wss://mainnet.base.org/ws");
        env::set_var("CONVEX_URL", "https://example.convex.cloud");

        let config = AppConfig::from_env().unwrap();

        assert_eq!(config.server.bind_addr, "0.0.0.0:8080".parse().unwrap());
        assert_eq!(config.server.frontend_url, "http://localhost:3000");
        assert_eq!(config.watcher.chain_id, 8453);
        assert_eq!(config.watcher.network, "base");
        assert_eq!(config.watcher.required_confirmations, 6);
        assert_eq!(config.watcher.polling_interval_secs, 10);
        assert_eq!(config.watcher.start_block, None);
        assert_eq!(config.convex.deployment_url, "https://example.convex.cloud");
        assert_eq!(config.convex.site_url, None);
        assert_eq!(config.convex.admin_key, None);
    }

    #[test]
    fn errors_when_required_values_are_missing() {
        let _guard = env_lock().lock().unwrap();
        clear_env();

        let result = AppConfig::from_env();
        assert!(result.is_err());
    }

    #[test]
    fn parses_optional_numeric_and_convex_values() {
        let _guard = env_lock().lock().unwrap();
        clear_env();

        env::set_var("BASE_RPC_URL", "https://mainnet.base.org");
        env::set_var("BASE_WSS_URL", "wss://mainnet.base.org/ws");
        env::set_var("WATCHER_START_BLOCK", "12345");
        env::set_var("REQUIRED_CONFIRMATIONS", "12");
        env::set_var("WATCHER_POLL_INTERVAL_SECS", "30");
        env::set_var("CONVEX_URL", "https://example.convex.cloud");
        env::set_var("CONVEX_SITE_URL", "https://example.convex.site");
        env::set_var("CONVEX_ADMIN_KEY", "test-admin-key");

        let config = AppConfig::from_env().unwrap();

        assert_eq!(config.watcher.start_block, Some(12345));
        assert_eq!(config.watcher.required_confirmations, 12);
        assert_eq!(config.watcher.polling_interval_secs, 30);
        assert_eq!(
            config.convex.site_url.as_deref(),
            Some("https://example.convex.site")
        );
        assert_eq!(config.convex.admin_key.as_deref(), Some("test-admin-key"));
    }
}
