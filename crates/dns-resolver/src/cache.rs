use hickory_proto::op::ResponseCode;
use hickory_proto::rr::{DNSClass, Name, Record, RecordType};
use moka::sync::Cache as MokaCache;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Cache key: (name, record type, class).
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct CacheKey {
    name: Name,
    rtype: RecordType,
    class: DNSClass,
}

/// A cached DNS response entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub records: Vec<Record>,
    pub response_code: ResponseCode,
    pub original_ttl: u32,
    inserted_at: Instant,
}

impl CacheEntry {
    /// Get the remaining TTL for this entry.
    pub fn remaining_ttl(&self) -> u32 {
        let elapsed = self.inserted_at.elapsed().as_secs() as u32;
        self.original_ttl.saturating_sub(elapsed)
    }

    /// Check if this entry has expired.
    pub fn is_expired(&self) -> bool {
        self.remaining_ttl() == 0
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: u64,
    pub hits: u64,
    pub misses: u64,
}

/// DNS response cache with TTL-based expiration and size limits.
pub struct DnsCache {
    cache: MokaCache<CacheKey, CacheEntry>,
    min_ttl: u32,
    max_ttl: u32,
    negative_ttl: u32,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl DnsCache {
    pub fn new(max_entries: u64, min_ttl: u32, max_ttl: u32, negative_ttl: u32) -> Self {
        let cache = MokaCache::builder()
            .max_capacity(max_entries)
            .build();

        Self {
            cache,
            min_ttl,
            max_ttl,
            negative_ttl,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Look up a cached entry. Returns None if not found or expired.
    pub fn get(&self, name: &Name, rtype: RecordType) -> Option<CacheEntry> {
        let key = CacheKey {
            name: name.clone(),
            rtype,
            class: DNSClass::IN,
        };

        match self.cache.get(&key) {
            Some(entry) if !CacheEntry::is_expired(&entry) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(entry)
            }
            Some(_) => {
                // Expired entry — remove it
                self.cache.invalidate(&key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// Insert a positive response into the cache.
    pub fn insert(&self, name: &Name, rtype: RecordType, records: Vec<Record>, ttl: u32) {
        let ttl = self.clamp_ttl(ttl);
        let key = CacheKey {
            name: name.clone(),
            rtype,
            class: DNSClass::IN,
        };
        let entry = CacheEntry {
            records,
            response_code: ResponseCode::NoError,
            original_ttl: ttl,
            inserted_at: Instant::now(),
        };
        self.cache.insert(key, entry);
    }

    /// Insert a negative response (NXDOMAIN or NODATA) into the cache.
    pub fn insert_negative(
        &self,
        name: &Name,
        rtype: RecordType,
        response_code: ResponseCode,
    ) {
        let ttl = self.negative_ttl.min(self.max_ttl).max(self.min_ttl);
        let key = CacheKey {
            name: name.clone(),
            rtype,
            class: DNSClass::IN,
        };
        let entry = CacheEntry {
            records: Vec::new(),
            response_code,
            original_ttl: ttl,
            inserted_at: Instant::now(),
        };
        self.cache.insert(key, entry);
    }

    /// Flush the entire cache.
    pub fn flush_all(&self) {
        self.cache.invalidate_all();
    }

    /// Flush all entries for a specific name.
    pub fn flush_name(&self, name: &Name) {
        // Moka doesn't support prefix invalidation, so we iterate
        // For common record types, invalidate each one
        for rtype in &[
            RecordType::A,
            RecordType::AAAA,
            RecordType::CNAME,
            RecordType::MX,
            RecordType::NS,
            RecordType::TXT,
            RecordType::SOA,
            RecordType::SRV,
            RecordType::PTR,
        ] {
            let key = CacheKey {
                name: name.clone(),
                rtype: *rtype,
                class: DNSClass::IN,
            };
            self.cache.invalidate(&key);
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.entry_count(),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }

    fn clamp_ttl(&self, ttl: u32) -> u32 {
        ttl.max(self.min_ttl).min(self.max_ttl)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::RData;
    use std::net::Ipv4Addr;

    fn make_record(name: &str, ip: Ipv4Addr) -> Record {
        Record::from_rdata(
            Name::from_ascii(name).unwrap(),
            300,
            RData::A(ip.into()),
        )
    }

    #[test]
    fn test_insert_and_get() {
        let cache = DnsCache::new(1000, 30, 86400, 300);
        let name = Name::from_ascii("example.com.").unwrap();
        let records = vec![make_record("example.com.", Ipv4Addr::new(1, 2, 3, 4))];

        cache.insert(&name, RecordType::A, records.clone(), 300);

        let entry = cache.get(&name, RecordType::A).unwrap();
        assert_eq!(entry.records.len(), 1);
        assert_eq!(entry.response_code, ResponseCode::NoError);
        assert!(entry.remaining_ttl() > 0);
    }

    #[test]
    fn test_miss() {
        let cache = DnsCache::new(1000, 30, 86400, 300);
        let name = Name::from_ascii("nonexistent.com.").unwrap();
        assert!(cache.get(&name, RecordType::A).is_none());
    }

    #[test]
    fn test_negative_caching() {
        let cache = DnsCache::new(1000, 30, 86400, 300);
        let name = Name::from_ascii("nxdomain.com.").unwrap();

        cache.insert_negative(&name, RecordType::A, ResponseCode::NXDomain);

        let entry = cache.get(&name, RecordType::A).unwrap();
        assert_eq!(entry.response_code, ResponseCode::NXDomain);
        assert!(entry.records.is_empty());
    }

    #[test]
    fn test_min_ttl() {
        let cache = DnsCache::new(1000, 60, 86400, 300);
        let name = Name::from_ascii("example.com.").unwrap();
        let records = vec![make_record("example.com.", Ipv4Addr::new(1, 2, 3, 4))];

        // Insert with TTL below minimum
        cache.insert(&name, RecordType::A, records, 5);

        let entry = cache.get(&name, RecordType::A).unwrap();
        // Should be clamped to min_ttl
        assert!(entry.original_ttl >= 60);
    }

    #[test]
    fn test_flush_all() {
        let cache = DnsCache::new(1000, 30, 86400, 300);
        let name = Name::from_ascii("example.com.").unwrap();
        let records = vec![make_record("example.com.", Ipv4Addr::new(1, 2, 3, 4))];
        cache.insert(&name, RecordType::A, records, 300);

        cache.flush_all();
        assert!(cache.get(&name, RecordType::A).is_none());
    }

    #[test]
    fn test_flush_name() {
        let cache = DnsCache::new(1000, 30, 86400, 300);
        let name = Name::from_ascii("example.com.").unwrap();
        let other = Name::from_ascii("other.com.").unwrap();
        cache.insert(
            &name,
            RecordType::A,
            vec![make_record("example.com.", Ipv4Addr::new(1, 2, 3, 4))],
            300,
        );
        cache.insert(
            &other,
            RecordType::A,
            vec![make_record("other.com.", Ipv4Addr::new(5, 6, 7, 8))],
            300,
        );

        cache.flush_name(&name);

        assert!(cache.get(&name, RecordType::A).is_none());
        assert!(cache.get(&other, RecordType::A).is_some());
    }

    #[test]
    fn test_stats() {
        let cache = DnsCache::new(1000, 30, 86400, 300);
        let name = Name::from_ascii("example.com.").unwrap();
        cache.insert(
            &name,
            RecordType::A,
            vec![make_record("example.com.", Ipv4Addr::new(1, 2, 3, 4))],
            300,
        );

        let _ = cache.get(&name, RecordType::A); // hit
        let _ = cache.get(&Name::from_ascii("miss.com.").unwrap(), RecordType::A); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        // Note: moka's entry_count() is eventually consistent,
        // so we just verify it's at least accessible
        assert!(stats.size <= 1);
    }
}
