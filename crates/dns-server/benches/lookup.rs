use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use dns_transport::QueryHandler;
use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{DNSClass, Name, RecordType};
use std::collections::HashMap;
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
mail IN A   192.0.2.200
ftp IN  CNAME www.example.com.
*.wild IN A 192.0.2.250
"#;

fn setup() -> (Arc<Router>, Vec<u8>) {
    let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("example.com.zone")).unwrap();
    let mut zones = HashMap::new();
    zones.insert(zone.origin.clone(), zone);
    let store = Arc::new(ZoneStore::new(zones));
    let acl = AclEngine::new(HashMap::new(), "any", "any");
    let router = Arc::new(Router::new(store, None, acl));

    let mut msg = Message::new();
    msg.set_id(0xABCD);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(false);
    let mut query = Query::new();
    query.set_name(Name::from_ascii("www.example.com.").unwrap());
    query.set_query_type(RecordType::A);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    let raw = msg.to_vec().unwrap();

    (router, raw)
}

fn bench_authoritative_lookup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (router, raw) = setup();
    let src = "127.0.0.1:12345".parse().unwrap();

    c.bench_function("authoritative_a_lookup", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resp = router.handle_query(black_box(&raw), src).await;
                black_box(resp);
            })
        })
    });
}

fn bench_nxdomain_lookup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (router, _) = setup();
    let src = "127.0.0.1:12345".parse().unwrap();

    let mut msg = Message::new();
    msg.set_id(0xABCD);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(false);
    let mut query = Query::new();
    query.set_name(Name::from_ascii("nonexistent.example.com.").unwrap());
    query.set_query_type(RecordType::A);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    let raw = msg.to_vec().unwrap();

    c.bench_function("nxdomain_lookup", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resp = router.handle_query(black_box(&raw), src).await;
                black_box(resp);
            })
        })
    });
}

fn bench_wildcard_lookup(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (router, _) = setup();
    let src = "127.0.0.1:12345".parse().unwrap();

    let mut msg = Message::new();
    msg.set_id(0xABCD);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(false);
    let mut query = Query::new();
    query.set_name(Name::from_ascii("anything.wild.example.com.").unwrap());
    query.set_query_type(RecordType::A);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    let raw = msg.to_vec().unwrap();

    c.bench_function("wildcard_lookup", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resp = router.handle_query(black_box(&raw), src).await;
                black_box(resp);
            })
        })
    });
}

criterion_group!(
    benches,
    bench_authoritative_lookup,
    bench_nxdomain_lookup,
    bench_wildcard_lookup,
);
criterion_main!(benches);
