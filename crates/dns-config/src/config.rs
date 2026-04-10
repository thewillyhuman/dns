use ipnet::IpNet;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    #[serde(default)]
    pub server: ListenConfig,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default)]
    pub zones: ZonesConfig,
    #[serde(default)]
    pub recursion: RecursionConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub dnssec: DnssecConfig,
    #[serde(default)]
    pub rrl: RrlConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub acls: HashMap<String, Vec<IpNet>>,
    #[serde(default)]
    pub policy: PolicyConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListenConfig {
    #[serde(default = "default_listen_udp")]
    pub listen_udp: Vec<SocketAddr>,
    #[serde(default = "default_listen_tcp")]
    pub listen_tcp: Vec<SocketAddr>,
    #[serde(default)]
    pub listen_dot: Vec<SocketAddr>,
    #[serde(default)]
    pub listen_doh: Vec<SocketAddr>,
    #[serde(default = "default_listen_http")]
    pub listen_http: SocketAddr,
    #[serde(default)]
    pub workers: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TlsConfig {
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZonesConfig {
    #[serde(default = "default_zone_dir")]
    pub directory: PathBuf,
    #[serde(default)]
    pub watch: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecursionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_root_hints")]
    pub root_hints: PathBuf,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default = "default_timeout")]
    pub timeout: String,
    #[serde(default = "default_retries")]
    pub retries: u32,
    #[serde(default = "default_true")]
    pub qname_minimization: bool,
    #[serde(default)]
    pub forwarding: ForwardingConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ForwardingConfig {
    #[serde(default)]
    pub zones: HashMap<String, Vec<SocketAddr>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    #[serde(default = "default_max_entries")]
    pub max_entries: u64,
    #[serde(default = "default_max_ttl")]
    pub max_ttl: u32,
    #[serde(default = "default_min_ttl")]
    pub min_ttl: u32,
    #[serde(default = "default_negative_ttl")]
    pub negative_ttl: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnssecConfig {
    #[serde(default)]
    pub enable_signing: bool,
    #[serde(default)]
    pub enable_validation: bool,
    #[serde(default = "default_key_dir")]
    pub key_directory: PathBuf,
    #[serde(default)]
    pub auto_rotate: bool,
    #[serde(default)]
    pub algorithms: DnssecAlgorithms,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DnssecAlgorithms {
    #[serde(default = "default_algorithm")]
    pub zsk: String,
    #[serde(default = "default_algorithm")]
    pub ksk: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RrlConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_rps")]
    pub responses_per_second: u32,
    #[serde(default = "default_slip")]
    pub slip: u32,
    #[serde(default = "default_ipv4_prefix")]
    pub ipv4_prefix_length: u8,
    #[serde(default = "default_ipv6_prefix")]
    pub ipv6_prefix_length: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    #[serde(default = "default_allow_recursion")]
    pub allow_recursion: String,
    #[serde(default = "default_allow_query")]
    pub allow_query: String,
}

// Default value functions

fn default_listen_udp() -> Vec<SocketAddr> {
    vec!["0.0.0.0:53".parse().unwrap(), "[::]:53".parse().unwrap()]
}

fn default_listen_tcp() -> Vec<SocketAddr> {
    vec!["0.0.0.0:53".parse().unwrap(), "[::]:53".parse().unwrap()]
}

fn default_listen_http() -> SocketAddr {
    "127.0.0.1:9153".parse().unwrap()
}

fn default_zone_dir() -> PathBuf {
    PathBuf::from("/etc/dns/zones")
}

fn default_root_hints() -> PathBuf {
    PathBuf::from("/etc/dns/root.hints")
}

fn default_max_depth() -> u32 {
    30
}

fn default_timeout() -> String {
    "2s".to_string()
}

fn default_retries() -> u32 {
    2
}

fn default_max_entries() -> u64 {
    1_000_000
}

fn default_max_ttl() -> u32 {
    86400
}

fn default_min_ttl() -> u32 {
    30
}

fn default_negative_ttl() -> u32 {
    300
}

fn default_key_dir() -> PathBuf {
    PathBuf::from("/etc/dns/keys")
}

fn default_algorithm() -> String {
    "ECDSAP256SHA256".to_string()
}

fn default_rps() -> u32 {
    5
}

fn default_slip() -> u32 {
    2
}

fn default_ipv4_prefix() -> u8 {
    24
}

fn default_ipv6_prefix() -> u8 {
    48
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

fn default_allow_recursion() -> String {
    "none".to_string()
}

fn default_allow_query() -> String {
    "any".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            listen_udp: default_listen_udp(),
            listen_tcp: default_listen_tcp(),
            listen_dot: Vec::new(),
            listen_doh: Vec::new(),
            listen_http: default_listen_http(),
            workers: 0,
        }
    }
}

impl Default for ZonesConfig {
    fn default() -> Self {
        Self {
            directory: default_zone_dir(),
            watch: false,
        }
    }
}

impl Default for RecursionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            root_hints: default_root_hints(),
            max_depth: default_max_depth(),
            timeout: default_timeout(),
            retries: default_retries(),
            qname_minimization: true,
            forwarding: ForwardingConfig::default(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_max_entries(),
            max_ttl: default_max_ttl(),
            min_ttl: default_min_ttl(),
            negative_ttl: default_negative_ttl(),
        }
    }
}

impl Default for DnssecConfig {
    fn default() -> Self {
        Self {
            enable_signing: false,
            enable_validation: false,
            key_directory: default_key_dir(),
            auto_rotate: false,
            algorithms: DnssecAlgorithms::default(),
        }
    }
}

impl Default for DnssecAlgorithms {
    fn default() -> Self {
        Self {
            zsk: default_algorithm(),
            ksk: default_algorithm(),
        }
    }
}

impl Default for RrlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            responses_per_second: default_rps(),
            slip: default_slip(),
            ipv4_prefix_length: default_ipv4_prefix(),
            ipv6_prefix_length: default_ipv6_prefix(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
        }
    }
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            allow_recursion: default_allow_recursion(),
            allow_query: default_allow_query(),
        }
    }
}

impl ServerConfig {
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_toml(&content)
    }

    pub fn parse_toml(s: &str) -> Result<Self, ConfigError> {
        let config: ServerConfig = toml::from_str(s)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.cache.max_ttl, 86400);
        assert_eq!(config.cache.min_ttl, 30);
        assert_eq!(config.cache.max_entries, 1_000_000);
        assert!(config.recursion.enabled);
        assert_eq!(config.recursion.max_depth, 30);
        assert!(!config.rrl.enabled);
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
[server]
listen_udp = ["0.0.0.0:5353", "[::]:5353"]
listen_tcp = ["0.0.0.0:5353"]
listen_http = "127.0.0.1:9153"
workers = 4

[tls]
cert_path = "/etc/dns/tls/cert.pem"
key_path = "/etc/dns/tls/key.pem"

[zones]
directory = "/etc/dns/zones"
watch = true

[recursion]
enabled = true
max_depth = 20
timeout = "3s"
retries = 3
qname_minimization = true

[cache]
max_entries = 500000
max_ttl = 43200
min_ttl = 60
negative_ttl = 600

[dnssec]
enable_signing = true
enable_validation = true

[rrl]
enabled = true
responses_per_second = 10
slip = 3

[logging]
level = "debug"
format = "json"

[policy]
allow_recursion = "cern-internal"
allow_query = "any"

[acls]
cern-internal = ["128.141.0.0/16", "128.142.0.0/16"]
"#;
        let config = ServerConfig::parse_toml(toml).unwrap();
        assert_eq!(config.server.workers, 4);
        assert!(config.zones.watch);
        assert_eq!(config.recursion.max_depth, 20);
        assert_eq!(config.cache.max_entries, 500000);
        assert!(config.dnssec.enable_signing);
        assert!(config.rrl.enabled);
        assert_eq!(config.rrl.responses_per_second, 10);
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.policy.allow_recursion, "cern-internal");
        assert!(config.acls.contains_key("cern-internal"));
        assert_eq!(config.acls["cern-internal"].len(), 2);
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml = "";
        let config = ServerConfig::parse_toml(toml).unwrap();
        assert_eq!(config.cache.max_ttl, 86400);
        assert!(config.recursion.enabled);
    }
}
