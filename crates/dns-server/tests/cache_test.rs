//! Integration tests for the DNS cache.

use dns_resolver::cache::DnsCache;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::net::Ipv4Addr;

fn make_a_record(name: &str, ip: Ipv4Addr) -> Record {
    Record::from_rdata(Name::from_ascii(name).unwrap(), 300, RData::A(ip.into()))
}

#[test]
fn test_cache_insert_and_get() {
    let cache = DnsCache::new(10000, 30, 86400, 300);
    let name = Name::from_ascii("test.example.com.").unwrap();
    let records = vec![make_a_record(
        "test.example.com.",
        Ipv4Addr::new(10, 0, 0, 1),
    )];

    cache.insert(&name, RecordType::A, records.clone(), 3600);

    let entry = cache.get(&name, RecordType::A).unwrap();
    assert_eq!(entry.records.len(), 1);
    assert!(entry.remaining_ttl() > 0);
    assert!(!entry.is_expired());
}

#[test]
fn test_cache_negative_entry() {
    let cache = DnsCache::new(10000, 30, 86400, 300);
    let name = Name::from_ascii("nx.example.com.").unwrap();

    cache.insert_negative(&name, RecordType::A, ResponseCode::NXDomain);

    let entry = cache.get(&name, RecordType::A).unwrap();
    assert_eq!(entry.response_code, ResponseCode::NXDomain);
    assert!(entry.records.is_empty());
}

#[test]
fn test_cache_flush_all() {
    let cache = DnsCache::new(10000, 30, 86400, 300);
    let name1 = Name::from_ascii("a.example.com.").unwrap();
    let name2 = Name::from_ascii("b.example.com.").unwrap();

    cache.insert(
        &name1,
        RecordType::A,
        vec![make_a_record("a.example.com.", Ipv4Addr::new(1, 1, 1, 1))],
        300,
    );
    cache.insert(
        &name2,
        RecordType::A,
        vec![make_a_record("b.example.com.", Ipv4Addr::new(2, 2, 2, 2))],
        300,
    );

    cache.flush_all();

    assert!(cache.get(&name1, RecordType::A).is_none());
    assert!(cache.get(&name2, RecordType::A).is_none());
}

#[test]
fn test_cache_flush_name() {
    let cache = DnsCache::new(10000, 30, 86400, 300);
    let name1 = Name::from_ascii("a.example.com.").unwrap();
    let name2 = Name::from_ascii("b.example.com.").unwrap();

    cache.insert(
        &name1,
        RecordType::A,
        vec![make_a_record("a.example.com.", Ipv4Addr::new(1, 1, 1, 1))],
        300,
    );
    cache.insert(
        &name2,
        RecordType::A,
        vec![make_a_record("b.example.com.", Ipv4Addr::new(2, 2, 2, 2))],
        300,
    );

    cache.flush_name(&name1);

    assert!(cache.get(&name1, RecordType::A).is_none());
    assert!(cache.get(&name2, RecordType::A).is_some());
}

#[test]
fn test_cache_ttl_clamping() {
    // min_ttl = 60, max_ttl = 3600
    let cache = DnsCache::new(10000, 60, 3600, 300);
    let name = Name::from_ascii("test.example.com.").unwrap();

    // Insert with very low TTL — should be clamped to min
    cache.insert(
        &name,
        RecordType::A,
        vec![make_a_record(
            "test.example.com.",
            Ipv4Addr::new(1, 2, 3, 4),
        )],
        5,
    );

    let entry = cache.get(&name, RecordType::A).unwrap();
    assert!(entry.original_ttl >= 60, "TTL should be clamped to min");
}

#[test]
fn test_cache_stats() {
    let cache = DnsCache::new(10000, 30, 86400, 300);
    let name = Name::from_ascii("test.example.com.").unwrap();
    cache.insert(
        &name,
        RecordType::A,
        vec![make_a_record(
            "test.example.com.",
            Ipv4Addr::new(1, 2, 3, 4),
        )],
        300,
    );

    let _ = cache.get(&name, RecordType::A); // hit
    let _ = cache.get(&Name::from_ascii("miss.com.").unwrap(), RecordType::A); // miss
    let _ = cache.get(&Name::from_ascii("miss2.com.").unwrap(), RecordType::A); // miss

    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 2);
}

#[test]
fn test_cache_different_types() {
    let cache = DnsCache::new(10000, 30, 86400, 300);
    let name = Name::from_ascii("test.example.com.").unwrap();

    // Insert A and AAAA records
    cache.insert(
        &name,
        RecordType::A,
        vec![make_a_record(
            "test.example.com.",
            Ipv4Addr::new(1, 2, 3, 4),
        )],
        300,
    );

    // A should exist, AAAA should not
    assert!(cache.get(&name, RecordType::A).is_some());
    assert!(cache.get(&name, RecordType::AAAA).is_none());
}
