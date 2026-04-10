//! Integration tests for Response Rate Limiting (RRL).

use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use dns_transport::rate_limit::RrlEngine;
use dns_transport::QueryHandler;
use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{DNSClass, Name, RecordType};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

const ZONE_DATA: &str = r#"
$ORIGIN example.com.
$TTL 3600
@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.example.com.
    IN  A   192.0.2.1
ns1 IN  A   192.0.2.10
www IN  A   192.0.2.100
"#;

fn build_query(name: &str, qtype: RecordType) -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(0xABCD);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(false);
    let mut query = Query::new();
    query.set_name(Name::from_ascii(name).unwrap());
    query.set_query_type(qtype);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    msg.to_vec().unwrap()
}

fn setup_router_with_rrl(rps: u32, slip: u32) -> Arc<Router> {
    let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("example.com.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = Arc::new(ZoneStore::new(zones));
    let acl = AclEngine::new(HashMap::new(), "any", "any");
    let rrl = Arc::new(RrlEngine::new(rps, slip, 24, 48));
    let router = Router::new(store, None, acl).with_rrl(rrl);
    Arc::new(router)
}

#[tokio::test]
async fn test_rrl_allows_under_limit() {
    let router = setup_router_with_rrl(100, 2);
    let raw = build_query("www.example.com.", RecordType::A);
    let src: SocketAddr = "10.0.0.1:12345".parse().unwrap();

    // Under the rate limit — all should succeed
    for _ in 0..10 {
        let resp = router.handle_query(&raw, src).await;
        assert!(resp.is_some(), "response should not be dropped under limit");
    }
}

#[tokio::test]
async fn test_rrl_drops_over_limit() {
    // Very low RPS so we can trigger it easily
    let router = setup_router_with_rrl(3, 0); // slip=0 means all drops, no truncate
    let raw = build_query("www.example.com.", RecordType::A);
    let src: SocketAddr = "10.0.0.1:12345".parse().unwrap();

    // Exhaust the bucket
    for _ in 0..3 {
        let _ = router.handle_query(&raw, src).await;
    }

    // Next should be dropped (None)
    let resp = router.handle_query(&raw, src).await;
    assert!(
        resp.is_none(),
        "response should be dropped after exceeding RRL"
    );
}

#[tokio::test]
async fn test_rrl_different_ips_independent() {
    let router = setup_router_with_rrl(3, 0);
    let raw = build_query("www.example.com.", RecordType::A);

    let src1: SocketAddr = "10.0.0.1:12345".parse().unwrap();
    let src2: SocketAddr = "10.0.1.1:12345".parse().unwrap(); // different /24

    // Exhaust src1's bucket
    for _ in 0..3 {
        let _ = router.handle_query(&raw, src1).await;
    }

    // src2 should still work (different prefix)
    let resp = router.handle_query(&raw, src2).await;
    assert!(
        resp.is_some(),
        "different /24 should have independent bucket"
    );
}
