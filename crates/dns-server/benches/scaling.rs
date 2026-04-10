//! Scaling benchmarks — measure how performance varies with zone size,
//! zone count, cache population, and reload cost.
//!
//! Zone size axis:  100, 100_000, 1_000_000 records (single zone)
//! Zone count axis: 1, 1_000, 10_000, 100_000 zones  (20 records each)
//! Cache axis:      100, 100_000, 1_000_000 entries
//!
//! WARNING: the largest cases (1M records, 100k zones) allocate significant
//! memory and take a while to set up. Total run time is ~15-20 minutes.

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
use std::time::Duration;

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
        let b2 = (i / 65536) % 256;
        let b3 = (i / 256) % 256;
        let b4 = i % 256;
        zone.push_str(&format!("host{i} IN  A   10.{b2}.{b3}.{b4}\n"));
    }
    zone
}

fn make_record(name: &str, ip: Ipv4Addr) -> Record {
    Record::from_rdata(Name::from_ascii(name).unwrap(), 300, RData::A(ip.into()))
}

fn make_router(zones: HashMap<Name, dns_authority::zone::Zone>) -> Arc<Router> {
    let store = Arc::new(ZoneStore::new(zones));
    let acl = AclEngine::new(HashMap::new(), "any", "any");
    Arc::new(Router::new(store, None, acl))
}

const SRC: &str = "127.0.0.1:12345";

// ─── zone size benchmarks ───────────────────────────────────────────

fn bench_lookup_by_zone_size(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let src: SocketAddr = SRC.parse().unwrap();

    let mut group = c.benchmark_group("lookup_by_zone_size");
    // Give large cases enough time
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for &record_count in &[100, 100_000, 1_000_000] {
        eprintln!("  [setup] generating zone with {record_count} records...");
        let zone_str = generate_zone("scale.example.com.", record_count);
        let zone = parse_zone_str(&zone_str, &PathBuf::from("scale.example.com.zone")).unwrap();
        let mut zones = HashMap::new();
        zones.insert(zone.origin.clone(), zone);
        let router = make_router(zones);
        drop(zone_str); // free the string memory

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

        // Query a name that doesn't exist (NXDOMAIN)
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
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for &zone_count in &[1, 1_000, 10_000, 100_000] {
        eprintln!("  [setup] generating {zone_count} zones (20 records each)...");
        let mut zones = HashMap::new();
        for i in 0..zone_count {
            let origin = format!("zone{i}.example.com.");
            let zone_str = generate_zone(&origin, 20);
            let zone = parse_zone_str(&zone_str, &PathBuf::from(format!("{origin}zone"))).unwrap();
            zones.insert(zone.origin.clone(), zone);
        }
        let router = make_router(zones);

        // Query a record in the last zone
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
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for &pop in &[100, 100_000, 1_000_000] {
        eprintln!("  [setup] populating cache with {pop} entries...");
        let cache = DnsCache::new(pop as u64 + 1000, 30, 86400, 300);

        for i in 0..pop {
            let name_str = format!("host{i}.cached.example.com.");
            let name = Name::from_ascii(&name_str).unwrap();
            let record = make_record(&name_str, Ipv4Addr::from(i as u32));
            cache.insert(&name, RecordType::A, vec![record], 300);
        }

        // Benchmark a hit
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
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for &pop in &[100, 100_000, 1_000_000] {
        eprintln!("  [setup] pre-filling cache to {pop} entries...");
        let cache = DnsCache::new(pop as u64 * 2, 30, 86400, 300);

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

// ─── reload benchmarks: zone size ───────────────────────────────────

fn bench_reload_by_zone_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload_by_zone_size");
    group.warm_up_time(Duration::from_secs(1));
    // Large zones need more time
    group.sample_size(10);

    for &record_count in &[100, 100_000, 1_000_000] {
        eprintln!("  [setup] generating zone string with {record_count} records...");
        let zone_str = generate_zone("reload.example.com.", record_count);

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

// ─── reload benchmarks: zone count ──────────────────────────────────

fn bench_reload_by_zone_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload_by_zone_count");
    group.warm_up_time(Duration::from_secs(1));
    group.sample_size(10);

    for &zone_count in &[1, 1_000, 10_000, 100_000] {
        eprintln!("  [setup] generating {zone_count} zone strings...");
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

// ─── reload benchmarks: atomic swap ─────────────────────────────────

fn bench_reload_swap(c: &mut Criterion) {
    let mut group = c.benchmark_group("reload_swap");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for &zone_count in &[1, 1_000, 10_000, 100_000] {
        eprintln!("  [setup] building store with {zone_count} zones for swap bench...");
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
    bench_reload_swap,
);
criterion_main!(benches);
