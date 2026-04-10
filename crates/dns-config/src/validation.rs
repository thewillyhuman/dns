use crate::config::ServerConfig;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("TLS cert_path is required when DoT or DoH listeners are configured")]
    MissingTlsCert,
    #[error("TLS key_path is required when DoT or DoH listeners are configured")]
    MissingTlsKey,
    #[error("recursion.max_depth must be > 0")]
    InvalidMaxDepth,
    #[error("rrl.slip must be >= 1")]
    InvalidSlip,
    #[error("cache.min_ttl ({min}) must be <= cache.max_ttl ({max})")]
    MinTtlExceedsMax { min: u32, max: u32 },
    #[error("policy.allow_recursion references unknown ACL group: {0}")]
    UnknownAclGroup(String),
    #[error("ipv4_prefix_length must be <= 32, got {0}")]
    InvalidIpv4Prefix(u8),
    #[error("ipv6_prefix_length must be <= 128, got {0}")]
    InvalidIpv6Prefix(u8),
}

pub fn validate(config: &ServerConfig) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    let needs_tls = !config.server.listen_dot.is_empty() || !config.server.listen_doh.is_empty();
    if needs_tls {
        if config.tls.cert_path.is_none() {
            errors.push(ValidationError::MissingTlsCert);
        }
        if config.tls.key_path.is_none() {
            errors.push(ValidationError::MissingTlsKey);
        }
    }

    if config.recursion.max_depth == 0 {
        errors.push(ValidationError::InvalidMaxDepth);
    }

    if config.rrl.enabled && config.rrl.slip < 1 {
        errors.push(ValidationError::InvalidSlip);
    }

    if config.cache.min_ttl > config.cache.max_ttl {
        errors.push(ValidationError::MinTtlExceedsMax {
            min: config.cache.min_ttl,
            max: config.cache.max_ttl,
        });
    }

    let policy_ref = &config.policy.allow_recursion;
    if policy_ref != "any" && policy_ref != "none" && !config.acls.contains_key(policy_ref) {
        errors.push(ValidationError::UnknownAclGroup(policy_ref.clone()));
    }

    if config.rrl.ipv4_prefix_length > 32 {
        errors.push(ValidationError::InvalidIpv4Prefix(
            config.rrl.ipv4_prefix_length,
        ));
    }

    if config.rrl.ipv6_prefix_length > 128 {
        errors.push(ValidationError::InvalidIpv6Prefix(
            config.rrl.ipv6_prefix_length,
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_default_config() {
        let config = ServerConfig::default();
        assert!(validate(&config).is_ok());
    }

    #[test]
    fn test_missing_tls_cert() {
        let mut config = ServerConfig::default();
        config.server.listen_dot = vec!["0.0.0.0:853".parse().unwrap()];
        let errors = validate(&config).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MissingTlsCert)));
    }

    #[test]
    fn test_invalid_max_depth() {
        let mut config = ServerConfig::default();
        config.recursion.max_depth = 0;
        let errors = validate(&config).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::InvalidMaxDepth)));
    }

    #[test]
    fn test_min_ttl_exceeds_max() {
        let mut config = ServerConfig::default();
        config.cache.min_ttl = 100;
        config.cache.max_ttl = 50;
        let errors = validate(&config).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::MinTtlExceedsMax { .. })));
    }

    #[test]
    fn test_unknown_acl_group() {
        let mut config = ServerConfig::default();
        config.policy.allow_recursion = "nonexistent".to_string();
        let errors = validate(&config).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, ValidationError::UnknownAclGroup(_))));
    }
}
