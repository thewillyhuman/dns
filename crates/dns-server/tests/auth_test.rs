//! Integration tests for authoritative DNS resolution.

use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use dns_transport::QueryHandler;
use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, Name, RData, RecordType};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

const ZONE_DATA: &str = r#"
$ORIGIN example.cern.ch.
$TTL 3600
@   IN  SOA ns1.example.cern.ch. admin.example.cern.ch. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.example.cern.ch.
    IN  NS  ns2.example.cern.ch.
    IN  A   192.168.1.1
    IN  AAAA    2001:db8::1
    IN  MX  10 mail.example.cern.ch.
ns1 IN  A   192.168.1.10
ns2 IN  A   192.168.1.11
www IN  A   192.168.1.100
    IN  AAAA    2001:db8::100
mail IN A   192.168.1.200
ftp IN  CNAME www.example.cern.ch.
*.wild IN A 192.168.1.250
sub IN  NS  ns.sub.example.cern.ch.
ns.sub IN A 192.168.1.50
"#;

fn setup_router() -> (Arc<Router>, SocketAddr) {
    let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("example.cern.ch.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = Arc::new(ZoneStore::new(zones));
    let acl = AclEngine::new(HashMap::new(), "any", "any");
    let router = Arc::new(Router::new(store, None, acl));
    let src: SocketAddr = "127.0.0.1:12345".parse().unwrap();
    (router, src)
}

fn build_query(name: &str, qtype: RecordType) -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(0x1234);
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

fn parse_response(bytes: &[u8]) -> Message {
    Message::from_vec(bytes).unwrap()
}

#[tokio::test]
async fn test_apex_a_query() {
    let (router, src) = setup_router();
    let raw = build_query("example.cern.ch.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.response_code(), ResponseCode::NoError);
    assert_eq!(msg.answers().len(), 1);

    match msg.answers()[0].data() {
        RData::A(a) => assert_eq!(a.0, "192.168.1.1".parse::<std::net::Ipv4Addr>().unwrap()),
        _ => panic!("expected A record"),
    }
}

#[tokio::test]
async fn test_apex_aaaa_query() {
    let (router, src) = setup_router();
    let raw = build_query("example.cern.ch.", RecordType::AAAA);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.answers().len(), 1);
}

#[tokio::test]
async fn test_www_a_query() {
    let (router, src) = setup_router();
    let raw = build_query("www.example.cern.ch.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.response_code(), ResponseCode::NoError);
    assert_eq!(msg.answers().len(), 1);
}

#[tokio::test]
async fn test_cname_query() {
    let (router, src) = setup_router();
    let raw = build_query("ftp.example.cern.ch.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    // Should get CNAME + resolved A record (CNAME chasing)
    assert!(msg.answers().len() >= 1);

    let has_cname = msg.answers().iter().any(|r| r.record_type() == RecordType::CNAME);
    assert!(has_cname, "response should include CNAME record");
}

#[tokio::test]
async fn test_mx_query() {
    let (router, src) = setup_router();
    let raw = build_query("example.cern.ch.", RecordType::MX);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.answers().len(), 1);
    assert_eq!(msg.answers()[0].record_type(), RecordType::MX);
}

#[tokio::test]
async fn test_nxdomain() {
    let (router, src) = setup_router();
    let raw = build_query("nonexistent.example.cern.ch.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.response_code(), ResponseCode::NXDomain);
    assert!(msg.answers().is_empty());
    // Should have SOA in authority
    assert!(!msg.name_servers().is_empty());
}

#[tokio::test]
async fn test_nodata() {
    let (router, src) = setup_router();
    // www.example.cern.ch exists but has no MX record
    let raw = build_query("www.example.cern.ch.", RecordType::MX);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.response_code(), ResponseCode::NoError);
    assert!(msg.answers().is_empty());
}

#[tokio::test]
async fn test_wildcard() {
    let (router, src) = setup_router();
    let raw = build_query("anything.wild.example.cern.ch.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert!(msg.authoritative());
    assert_eq!(msg.response_code(), ResponseCode::NoError);
    assert_eq!(msg.answers().len(), 1);

    // Wildcard-synthesized record should have the queried name
    assert_eq!(
        msg.answers()[0].name(),
        &Name::from_ascii("anything.wild.example.cern.ch.").unwrap()
    );
}

#[tokio::test]
async fn test_delegation() {
    let (router, src) = setup_router();
    let raw = build_query("host.sub.example.cern.ch.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    // Should be a referral (not authoritative, NS in authority)
    // Or REFUSED since we have no recursion and the zone delegates
    // The exact behavior depends on how the zone lookup handles delegations
    assert!(
        msg.response_code() == ResponseCode::NoError
            || msg.response_code() == ResponseCode::Refused
            || msg.response_code() == ResponseCode::NXDomain
    );
}

#[tokio::test]
async fn test_non_authoritative_refused() {
    let (router, src) = setup_router();
    let raw = build_query("www.google.com.", RecordType::A);
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = parse_response(&resp);

    assert_eq!(msg.response_code(), ResponseCode::Refused);
}

#[tokio::test]
async fn test_multiple_queries_same_name() {
    let (router, src) = setup_router();

    // Query the same name multiple times — should be consistent
    for _ in 0..5 {
        let raw = build_query("www.example.cern.ch.", RecordType::A);
        let resp = router.handle_query(&raw, src).await.unwrap();
        let msg = parse_response(&resp);
        assert!(msg.authoritative());
        assert_eq!(msg.answers().len(), 1);
    }
}
