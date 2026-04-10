use ipnet::IpNet;
use std::collections::HashMap;
use std::net::IpAddr;

/// Access control engine for DNS queries.
#[derive(Debug, Clone)]
pub struct AclEngine {
    named_groups: HashMap<String, Vec<IpNet>>,
    allow_recursion: AclPolicy,
    allow_query: AclPolicy,
}

#[derive(Debug, Clone)]
pub enum AclPolicy {
    Any,
    None,
    Groups(Vec<String>),
}

impl AclEngine {
    pub fn new(
        named_groups: HashMap<String, Vec<IpNet>>,
        allow_recursion: &str,
        allow_query: &str,
    ) -> Self {
        Self {
            named_groups,
            allow_recursion: parse_policy(allow_recursion),
            allow_query: parse_policy(allow_query),
        }
    }

    /// Check if recursion is allowed for a source IP.
    pub fn is_recursion_allowed(&self, src: &IpAddr) -> bool {
        self.matches_policy(src, &self.allow_recursion)
    }

    /// Check if querying is allowed for a source IP.
    pub fn is_query_allowed(&self, src: &IpAddr) -> bool {
        self.matches_policy(src, &self.allow_query)
    }

    fn matches_policy(&self, src: &IpAddr, policy: &AclPolicy) -> bool {
        match policy {
            AclPolicy::Any => true,
            AclPolicy::None => false,
            AclPolicy::Groups(groups) => {
                for group_name in groups {
                    if let Some(nets) = self.named_groups.get(group_name) {
                        for net in nets {
                            if net.contains(src) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
        }
    }
}

fn parse_policy(s: &str) -> AclPolicy {
    match s {
        "any" => AclPolicy::Any,
        "none" => AclPolicy::None,
        group => AclPolicy::Groups(vec![group.to_string()]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> AclEngine {
        let mut groups = HashMap::new();
        groups.insert(
            "internal".to_string(),
            vec![
                "10.0.0.0/8".parse().unwrap(),
                "192.168.0.0/16".parse().unwrap(),
            ],
        );
        AclEngine::new(groups, "internal", "any")
    }

    #[test]
    fn test_query_allowed_any() {
        let engine = test_engine();
        assert!(engine.is_query_allowed(&"1.2.3.4".parse().unwrap()));
        assert!(engine.is_query_allowed(&"10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_recursion_internal_only() {
        let engine = test_engine();
        assert!(engine.is_recursion_allowed(&"10.0.0.1".parse().unwrap()));
        assert!(engine.is_recursion_allowed(&"192.168.1.1".parse().unwrap()));
        assert!(!engine.is_recursion_allowed(&"1.2.3.4".parse().unwrap()));
    }

    #[test]
    fn test_none_policy() {
        let engine = AclEngine::new(HashMap::new(), "none", "none");
        assert!(!engine.is_recursion_allowed(&"10.0.0.1".parse().unwrap()));
        assert!(!engine.is_query_allowed(&"10.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_any_policy() {
        let engine = AclEngine::new(HashMap::new(), "any", "any");
        assert!(engine.is_recursion_allowed(&"1.2.3.4".parse().unwrap()));
        assert!(engine.is_query_allowed(&"1.2.3.4".parse().unwrap()));
    }
}
