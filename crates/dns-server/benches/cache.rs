use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dns_resolver::cache::DnsCache;
use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::net::Ipv4Addr;

fn make_record(name: &str, ip: Ipv4Addr) -> Record {
    Record::from_rdata(Name::from_ascii(name).unwrap(), 300, RData::A(ip.into()))
}

fn bench_cache_insert(c: &mut Criterion) {
    let cache = DnsCache::new(100_000, 30, 86400, 300);

    c.bench_function("cache_insert", |b| {
        let mut i = 0u32;
        b.iter(|| {
            let name_str = format!("host{}.example.com.", i);
            let name = Name::from_ascii(&name_str).unwrap();
            let record = make_record(&name_str, Ipv4Addr::from(i));
            cache.insert(black_box(&name), RecordType::A, vec![record], 300);
            i = i.wrapping_add(1);
        })
    });
}

fn bench_cache_get_hit(c: &mut Criterion) {
    let cache = DnsCache::new(100_000, 30, 86400, 300);
    let name = Name::from_ascii("cached.example.com.").unwrap();
    cache.insert(
        &name,
        RecordType::A,
        vec![make_record(
            "cached.example.com.",
            Ipv4Addr::new(1, 2, 3, 4),
        )],
        300,
    );

    c.bench_function("cache_get_hit", |b| {
        b.iter(|| {
            let entry = cache.get(black_box(&name), RecordType::A);
            black_box(entry);
        })
    });
}

fn bench_cache_get_miss(c: &mut Criterion) {
    let cache = DnsCache::new(100_000, 30, 86400, 300);
    let name = Name::from_ascii("miss.example.com.").unwrap();

    c.bench_function("cache_get_miss", |b| {
        b.iter(|| {
            let entry = cache.get(black_box(&name), RecordType::A);
            black_box(entry);
        })
    });
}

criterion_group!(
    benches,
    bench_cache_insert,
    bench_cache_get_hit,
    bench_cache_get_miss
);
criterion_main!(benches);
