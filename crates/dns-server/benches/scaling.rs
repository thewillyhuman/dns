//! Scaling benchmarks — measure how performance varies with zone size,
//! zone count, and cache population.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_resolver::cache::DnsCache;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use dns_transport::QueryHandler;
use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

// ─── helpers ────────────────────────────────────────────────────────

fn build_query_wire(name: &str) -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(0x1234);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(false);
    let mut query = Query::new();
    query.set_name(Name::from_ascii(name).unwrap());
    query.set_query_type(RecordType::A);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    msg.to_vec().unwrap()
}

/// Generate a zone string with `n` A records under `$ORIGIN <origin>.`
fn generate_zone(origin: &str, record_count: usize) -> String {
    let mut zone = format!(
        "$ORIGIN {origin}\n\
         $TTL 3600\n\
         @   IN  SOA ns1.{origin} admin.{origin} (\n\
                     2024010101 3600 900 604800 86400 )\n\
             IN  NS  ns1.{origin}\n\
             IN  A   10.0.0.1\n\
         ns1 IN  A   10.0.0.2\n"
    );
    for i in 0..record_count {
        let octet3 = (i / 256) % 256;
        let octet4 = i % 256;
        zone.push_str(&format!("host{i} IN  A   10.1.{octet3}.{octet4}\n"));
    }
    zone
}

fn make_record(name: &str, ip: Ipv4Addr) -> Record {
    Record::from_rdata(Name::from_ascii(name).unwrap(), 300, RData::A(ip.into()))
}

const SRC: &str = "127.0.0.1:12345";

// ─── zone size benchmarks ───────────────────────────────────────────

fn bench_lookup_by_zone_size(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let src: SocketAddr = SRC.parse().unwrap();

    let mut group = c.benchmark_group("lookup_by_zone_size");
    for &record_count in &[10, 100, 1_000, 10_000] {
        let zone_str = generate_zone("scale.example.com.", record_count);
        let zone = parse_zone_str(&zone_str, &PathBuf::from("scale.example.com.zone")).unwrap();
        let mut zones = HashMap::new();
        zones.insert(zone.origin.clone(), zone);
        let store = Arc::new(ZoneStore::new(zones));
        let acl = AclEngine::new(HashMap::new(), "any", "any");
        let router = Arc::new(Router::new(store, None, acl));

        // Query an existing record (roughly in the middle)
        let target = format!("host{}.scale.example.com.", record_count / 2);
        let raw = build_query_wire(&target);

        group.bench_with_input(
            BenchmarkId::new("exact_hit", record_count),
            &record_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let resp = router.handle_query(black_box(&raw), src).await;
                        black_box(resp);
                    })
                })
            },
        );

        // Query a name that doesn't exist (NXDOMAIN) — exercises full scan
        let nxraw = build_query_wire("nonexistent.scale.example.com.");
        group.bench_with_input(
            BenchmarkId::new("nxdomain", record_count),
            &record_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let resp = router.handle_query(black_box(&nxraw), src).await;
                        black_box(resp);
                    })
                })
            },
        );
    }
    group.finish();
}

// ─── zone count benchmarks ──────────────────────────────────────────

fn bench_lookup_by_zone_count(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let src: SocketAddr = SRC.parse().unwrap();

    let mut group = c.benchmark_group("lookup_by_zone_count");
    for &zone_count in &[1, 10, 100, 1_000, 10_000] {
        let mut zones = HashMap::new();
        for i in 0..zone_count {
            let origin = format!("zone{i}.example.com.");
            let zone_str = generate_zone(&origin, 20);
            let zone = parse_zone_str(&zone_str, &PathBuf::from(format!("{origin}zone"))).unwrap();
            zones.insert(zone.origin.clone(), zone);
        }
        let store = Arc::new(ZoneStore::new(zones));
        let acl = AclEngine::new(HashMap::new(), "any", "any");
        let router = Arc::new(Router::new(store, None, acl));

        // Query a record in the last zone (worst-case zone lookup)
        let target = format!("host10.zone{}.example.com.", zone_count - 1);
        let raw = build_query_wire(&target);

        group.bench_with_input(
            BenchmarkId::new("hit_last_zone", zone_count),
            &zone_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let resp = router.handle_query(black_box(&raw), src).await;
                        black_box(resp);
                    })
                })
            },
        );

        // Query a name outside all zones (no match)
        let miss_raw = build_query_wire("www.notloaded.net.");
        group.bench_with_input(
            BenchmarkId::new("miss_no_zone", zone_count),
            &zone_count,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let resp = router.handle_query(black_box(&miss_raw), src).await;
                        black_box(resp);
                    })
                })
            },
        );
    }
    group.finish();
}

// ─── cache scaling benchmarks ───────────────────────────────────────

fn bench_cache_get_by_population(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_get_by_population");
    for &pop in &[100, 1_000, 10_000, 100_000, 500_000] {
        let cache = DnsCache::new(pop as u64 + 1000, 30, 86400, 300);

        // Fill the cache
        for i in 0..pop {
            let name_str = format!("host{i}.cached.example.com.");
            let name = Name::from_ascii(&name_str).unwrap();
            let record = make_record(&name_str, Ipv4Addr::from(i as u32));
            cache.insert(&name, RecordType::A, vec![record], 300);
        }

        // Benchmark a hit (query a name that exists)
        let hit_name = Name::from_ascii(format!("host{}.cached.example.com.", pop / 2)).unwrap();
        group.bench_with_input(BenchmarkId::new("hit", pop), &pop, |b, _| {
            b.iter(|| {
                let entry = cache.get(black_box(&hit_name), RecordType::A);
                black_box(entry);
            })
        });

        // Benchmark a miss
        let miss_name = Name::from_ascii("notcached.example.com.").unwrap();
        group.bench_with_input(BenchmarkId::new("miss", pop), &pop, |b, _| {
            b.iter(|| {
                let entry = cache.get(black_box(&miss_name), RecordType::A);
                black_box(entry);
            })
        });
    }
    group.finish();
}

fn bench_cache_insert_by_population(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_insert_by_population");
    for &pop in &[100, 1_000, 10_000, 100_000, 500_000] {
        let cache = DnsCache::new(pop as u64 + 1000, 30, 86400, 300);

        // Pre-fill
        for i in 0..pop {
            let name_str = format!("host{i}.fill.example.com.");
            let name = Name::from_ascii(&name_str).unwrap();
            let record = make_record(&name_str, Ipv4Addr::from(i as u32));
            cache.insert(&name, RecordType::A, vec![record], 300);
        }

        group.bench_with_input(BenchmarkId::new("insert", pop), &pop, |b, _| {
            let mut i = pop as u32;
            b.iter(|| {
                let name_str = format!("new{i}.fill.example.com.");
                let name = Name::from_ascii(&name_str).unwrap();
                let record = make_record(&name_str, Ipv4Addr::from(i));
                cache.insert(black_box(&name), RecordType::A, vec![record], 300);
                i = i.wrapping_add(1);
            })
        });
    }
    group.finish();
}

// ─── reload benchmarks ──────────────────────────────────────────────

fn bench_reload_by_zone_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload_by_zone_size");
    for &record_count in &[10, 100, 1_000, 10_000] {
        let zone_str = generate_zone("reload.example.com.", record_count);

        // Measure building a new ZoneStore from parsed zone data
        group.bench_with_input(
            BenchmarkId::new("parse_and_build", record_count),
            &record_count,
            |b, _| {
                b.iter(|| {
                    let zone = parse_zone_str(
                        black_box(&zone_str),
                        &PathBuf::from("reload.example.com.zone"),
                    )
                    .unwrap();
                    let mut zones = HashMap::new();
                    zones.insert(zone.origin.clone(), zone);
                    let store = ZoneStore::new(zones);
                    black_box(store);
                })
            },
        );
    }
    group.finish();
}

fn bench_reload_by_zone_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload_by_zone_count");
    for &zone_count in &[1, 10, 100, 1_000] {
        // Pre-generate all zone strings
        let zone_strings: Vec<(String, String)> = (0..zone_count)
            .map(|i| {
                let origin = format!("zone{i}.reload.example.com.");
                let data = generate_zone(&origin, 20);
                (origin, data)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("parse_and_build_all", zone_count),
            &zone_count,
            |b, _| {
                b.iter(|| {
                    let mut zones = HashMap::new();
                    for (origin, data) in &zone_strings {
                        let zone = parse_zone_str(
                            black_box(data),
                            &PathBuf::from(format!("{origin}zone")),
                        )
                        .unwrap();
                        zones.insert(zone.origin.clone(), zone);
                    }
                    let store = ZoneStore::new(zones);
                    black_box(store);
                })
            },
        );
    }
    group.finish();
}

fn bench_reload_swap_with_zones(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload_swap");
    for &zone_count in &[1, 100, 1_000, 10_000] {
        // Build the initial store with zone_count zones
        let mut zones = HashMap::new();
        for i in 0..zone_count {
            let origin = format!("zone{i}.swap.example.com.");
            let zone_str = generate_zone(&origin, 20);
            let zone = parse_zone_str(&zone_str, &PathBuf::from(format!("{origin}zone"))).unwrap();
            zones.insert(zone.origin.clone(), zone);
        }
        let store = ZoneStore::new(zones);

        // Pre-parse the replacement zone
        let new_zone_str = generate_zone("zone0.swap.example.com.", 20);
        let template_zone =
            parse_zone_str(&new_zone_str, &PathBuf::from("zone0.swap.example.com.zone")).unwrap();

        // Benchmark the atomic swap only (zone already parsed)
        group.bench_with_input(
            BenchmarkId::new("swap_only", zone_count),
            &zone_count,
            |b, _| {
                b.iter(|| {
                    store.swap_zone(black_box(template_zone.clone()));
                })
            },
        );

        // Benchmark parse + swap (end-to-end single zone reload)
        group.bench_with_input(
            BenchmarkId::new("parse_and_swap", zone_count),
            &zone_count,
            |b, _| {
                b.iter(|| {
                    let zone = parse_zone_str(
                        black_box(&new_zone_str),
                        &PathBuf::from("zone0.swap.example.com.zone"),
                    )
                    .unwrap();
                    store.swap_zone(zone);
                })
            },
        );
    }
    group.finish();
}

// ─── criterion groups ───────────────────────────────────────────────

criterion_group!(
    benches,
    bench_lookup_by_zone_size,
    bench_lookup_by_zone_count,
    bench_cache_get_by_population,
    bench_cache_insert_by_population,
    bench_reload_by_zone_size,
    bench_reload_by_zone_count,
    bench_reload_swap_with_zones,
);
criterion_main!(benches);
