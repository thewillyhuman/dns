use crate::loader::{self, ZoneLoadError};
use crate::lookup;
use crate::negative::AuthResponse;
use crate::zone::Zone;
use arc_swap::ArcSwap;
use hickory_proto::rr::{Name, RecordType};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Thread-safe, atomically-swappable store of all authoritative zones.
pub struct ZoneStore {
    inner: ArcSwap<ZoneStoreInner>,
}

struct ZoneStoreInner {
    zones: HashMap<Name, Arc<Zone>>,
}

impl ZoneStore {
    /// Create a new zone store from a map of zones.
    pub fn new(zones: HashMap<Name, Zone>) -> Self {
        let inner = ZoneStoreInner {
            zones: zones.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
        };
        Self {
            inner: ArcSwap::new(Arc::new(inner)),
        }
    }

    /// Create an empty zone store.
    pub fn empty() -> Self {
        Self::new(HashMap::new())
    }

    /// Load zones from a directory and create a new store.
    pub fn from_directory(dir: &Path) -> Result<Self, ZoneLoadError> {
        let zones = loader::load_zone_directory(dir)?;
        Ok(Self::new(zones))
    }

    /// Find the most specific authoritative zone for a query name.
    pub fn find_zone(&self, qname: &Name) -> Option<Arc<Zone>> {
        let guard = self.inner.load();
        // Walk from the full qname upward, looking for the most specific zone
        let mut current = qname.clone();
        loop {
            if let Some(zone) = guard.zones.get(&current) {
                return Some(Arc::clone(zone));
            }
            match dns_protocol::name::parent(&current) {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    }

    /// Check if we are authoritative for a name.
    pub fn is_authoritative_for(&self, name: &Name) -> bool {
        self.find_zone(name).is_some()
    }

    /// Perform an authoritative lookup.
    pub fn lookup(&self, qname: &Name, qtype: RecordType) -> Option<AuthResponse> {
        let zone = self.find_zone(qname)?;
        Some(lookup::resolve_query(&zone, qname, qtype))
    }

    /// Atomically reload all zones from a directory.
    pub fn reload_all(&self, dir: &Path) -> Result<ReloadResult, ZoneLoadError> {
        let zones = loader::load_zone_directory(dir)?;
        let count = zones.len();
        let inner = ZoneStoreInner {
            zones: zones.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
        };
        self.inner.store(Arc::new(inner));
        Ok(ReloadResult {
            zones_loaded: count,
            errors: Vec::new(),
        })
    }

    /// Reload a single zone from a file.
    pub fn reload_zone(&self, path: &Path) -> Result<(), ZoneLoadError> {
        let zone = loader::load_zone_file(path)?;
        let origin = zone.origin.clone();
        let guard = self.inner.load();
        let mut new_zones = guard.zones.clone();
        new_zones.insert(origin, Arc::new(zone));
        self.inner
            .store(Arc::new(ZoneStoreInner { zones: new_zones }));
        Ok(())
    }

    /// Swap in a pre-parsed zone, replacing any existing zone with the same origin.
    pub fn swap_zone(&self, zone: Zone) {
        let origin = zone.origin.clone();
        let guard = self.inner.load();
        let mut new_zones = guard.zones.clone();
        new_zones.insert(origin, Arc::new(zone));
        self.inner
            .store(Arc::new(ZoneStoreInner { zones: new_zones }));
    }

    /// Get the list of all zone names.
    pub fn zone_names(&self) -> Vec<Name> {
        let guard = self.inner.load();
        guard.zones.keys().cloned().collect()
    }

    /// Get zone info for a specific zone.
    pub fn get_zone(&self, name: &Name) -> Option<Arc<Zone>> {
        let guard = self.inner.load();
        guard.zones.get(name).cloned()
    }

    /// Total number of zones.
    pub fn zone_count(&self) -> usize {
        let guard = self.inner.load();
        guard.zones.len()
    }
}

#[derive(Debug)]
pub struct ReloadResult {
    pub zones_loaded: usize,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::parse_zone_str;
    use hickory_proto::op::ResponseCode;
    use std::path::PathBuf;

    const TEST_ZONE: &str = r#"
$ORIGIN example.com.
$TTL 3600
@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.example.com.
    IN  A   192.0.2.1
ns1 IN  A   192.0.2.10
www IN  A   192.0.2.100
"#;

    fn test_store() -> ZoneStore {
        let zone = parse_zone_str(TEST_ZONE, &PathBuf::from("example.com.zone")).unwrap();
        let mut zones = HashMap::new();
        zones.insert(zone.origin.clone(), zone);
        ZoneStore::new(zones)
    }

    #[test]
    fn test_find_zone() {
        let store = test_store();
        let qname = Name::from_ascii("www.example.com.").unwrap();
        assert!(store.find_zone(&qname).is_some());

        let outside = Name::from_ascii("www.other.com.").unwrap();
        assert!(store.find_zone(&outside).is_none());
    }

    #[test]
    fn test_is_authoritative() {
        let store = test_store();
        assert!(store.is_authoritative_for(&Name::from_ascii("www.example.com.").unwrap()));
        assert!(!store.is_authoritative_for(&Name::from_ascii("www.other.com.").unwrap()));
    }

    #[test]
    fn test_lookup() {
        let store = test_store();
        let qname = Name::from_ascii("www.example.com.").unwrap();
        let resp = store.lookup(&qname, RecordType::A).unwrap();
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert_eq!(resp.answers.len(), 1);
    }

    #[test]
    fn test_zone_count() {
        let store = test_store();
        assert_eq!(store.zone_count(), 1);
    }

    #[test]
    fn test_zone_names() {
        let store = test_store();
        let names = store.zone_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], Name::from_ascii("example.com.").unwrap());
    }
}
