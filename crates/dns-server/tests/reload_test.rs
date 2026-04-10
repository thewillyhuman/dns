//! Integration tests for hot zone reload.

use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use hickory_proto::rr::{Name, RecordType};
use std::collections::HashMap;
use std::path::PathBuf;

const ZONE_V1: &str = r#"
$ORIGIN example.com.
$TTL 3600
@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.example.com.
    IN  A   192.0.2.1
ns1 IN  A   192.0.2.10
www IN  A   192.0.2.100
"#;

const ZONE_V2: &str = r#"
$ORIGIN example.com.
$TTL 3600
@   IN  SOA ns1.example.com. admin.example.com. (
            2024010102 3600 900 604800 86400 )
    IN  NS  ns1.example.com.
    IN  A   192.0.2.1
ns1 IN  A   192.0.2.10
www IN  A   192.0.2.200
new IN  A   192.0.2.50
"#;

#[test]
fn test_zone_store_query_consistency() {
    let zone = parse_zone_str(ZONE_V1, &PathBuf::from("example.com.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = ZoneStore::new(zones);

    let origin = Name::from_ascii("example.com.").unwrap();

    // Initial state
    assert!(store.is_authoritative_for(&Name::from_ascii("www.example.com.").unwrap()));
    let resp = store.lookup(
        &Name::from_ascii("www.example.com.").unwrap(),
        RecordType::A,
    );
    assert!(resp.is_some());

    // Zone names should include example.com
    let names = store.zone_names();
    assert!(names.contains(&origin));
}

#[test]
fn test_zone_store_multiple_zones() {
    let zone1 = parse_zone_str(ZONE_V1, &PathBuf::from("example.com.zone")).unwrap();

    let zone2_data = r#"
$ORIGIN other.org.
$TTL 3600
@   IN  SOA ns1.other.org. admin.other.org. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.other.org.
    IN  A   10.0.0.1
ns1 IN  A   10.0.0.10
"#;
    let zone2 = parse_zone_str(zone2_data, &PathBuf::from("other.org.zone")).unwrap();

    let mut zones = HashMap::new();
    zones.insert(zone1.origin.clone(), zone1);
    zones.insert(zone2.origin.clone(), zone2);
    let store = ZoneStore::new(zones);

    assert!(store.is_authoritative_for(&Name::from_ascii("www.example.com.").unwrap()));
    assert!(store.is_authoritative_for(&Name::from_ascii("ns1.other.org.").unwrap()));
    assert!(!store.is_authoritative_for(&Name::from_ascii("www.google.com.").unwrap()));
}

#[test]
fn test_serial_number_tracking() {
    let zone = parse_zone_str(ZONE_V1, &PathBuf::from("example.com.zone")).unwrap();
    assert_eq!(zone.serial(), 2024010101);

    let zone_v2 = parse_zone_str(ZONE_V2, &PathBuf::from("example.com.zone")).unwrap();
    assert_eq!(zone_v2.serial(), 2024010102);
}
