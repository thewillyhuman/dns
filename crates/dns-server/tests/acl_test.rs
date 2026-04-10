//! Integration tests for ACL enforcement.

use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use dns_transport::QueryHandler;
use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, Name, RecordType};
use ipnet::IpNet;
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
    msg.set_id(0x5678);
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

#[tokio::test]
async fn test_acl_allow_all() {
    let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("example.com.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = Arc::new(ZoneStore::new(zones));
    let acl = AclEngine::new(HashMap::new(), "any", "any");
    let router = Arc::new(Router::new(store, None, acl));

    let raw = build_query("www.example.com.", RecordType::A);
    let src: SocketAddr = "10.0.0.1:12345".parse().unwrap();
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = Message::from_vec(&resp).unwrap();

    assert_eq!(msg.response_code(), ResponseCode::NoError);
}

#[tokio::test]
async fn test_acl_deny_all_queries() {
    let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("example.com.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = Arc::new(ZoneStore::new(zones));
    let acl = AclEngine::new(HashMap::new(), "any", "none");
    let router = Arc::new(Router::new(store, None, acl));

    let raw = build_query("www.example.com.", RecordType::A);
    let src: SocketAddr = "10.0.0.1:12345".parse().unwrap();
    let resp = router.handle_query(&raw, src).await.unwrap();
    let msg = Message::from_vec(&resp).unwrap();

    assert_eq!(msg.response_code(), ResponseCode::Refused);
}

#[tokio::test]
async fn test_acl_group_based() {
    let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("example.com.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = Arc::new(ZoneStore::new(zones));

    let mut acl_groups = HashMap::new();
    acl_groups.insert(
        "internal".to_string(),
        vec!["10.0.0.0/8".parse::<IpNet>().unwrap()],
    );

    let acl = AclEngine::new(acl_groups, "any", "internal");
    let router = Arc::new(Router::new(store, None, acl));

    // Internal source — should be allowed
    let raw = build_query("www.example.com.", RecordType::A);
    let src_internal: SocketAddr = "10.1.2.3:12345".parse().unwrap();
    let resp = router.handle_query(&raw, src_internal).await.unwrap();
    let msg = Message::from_vec(&resp).unwrap();
    assert_eq!(msg.response_code(), ResponseCode::NoError);

    // External source — should be refused
    let src_external: SocketAddr = "203.0.113.1:12345".parse().unwrap();
    let resp = router.handle_query(&raw, src_external).await.unwrap();
    let msg = Message::from_vec(&resp).unwrap();
    assert_eq!(msg.response_code(), ResponseCode::Refused);
}
