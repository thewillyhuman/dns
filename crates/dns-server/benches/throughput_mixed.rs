//! Mixed-workload concurrent throughput benchmark.
//!
//! Simulates a realistic production traffic mix:
//!   - 20% authoritative zone hits   (in-memory, fast)
//!   - 70% recursive cache hits      (pre-warmed cache)
//!   - 10% recursive cache misses    (forwarded to a stub upstream with synthetic latency)
//!
//! A stub upstream DNS server runs on localhost and answers every query with a
//! canned A record after a configurable random delay (uniform 2–20 ms by
//! default), modelling real-world upstream RTT.

use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_config::config::{CacheConfig, ForwardingConfig, RecursionConfig};
use dns_resolver::Resolver;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use rand::Rng;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

// ─── zone data (authoritative names) ────────────────────────────────

const ZONE_DATA: &str = r#"
$ORIGIN cern.ch.
$TTL 3600
@       IN  SOA ns1.cern.ch. admin.cern.ch. (
                2024010101 3600 900 604800 86400 )
        IN  NS  ns1.cern.ch.
        IN  A   192.168.0.1
ns1     IN  A   192.168.0.10
www     IN  A   192.168.0.100
mail    IN  A   192.168.0.200
lhc     IN  A   192.168.0.50
atlas   IN  A   192.168.0.60
cms     IN  A   192.168.0.70
alice   IN  A   192.168.0.80
lhcb    IN  A   192.168.0.90
"#;

// ─── stub upstream ──────────────────────────────────────────────────

/// Start a fake upstream DNS server that answers every query with a
/// canned A record after a random delay in `[delay_min, delay_max)`.
async fn start_stub_upstream(
    delay_min: Duration,
    delay_max: Duration,
    stop: Arc<AtomicBool>,
) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        while !stop.load(Ordering::Relaxed) {
            let recv = tokio::time::timeout(Duration::from_millis(200), socket.recv_from(&mut buf));
            let (len, src) = match recv.await {
                Ok(Ok(v)) => v,
                _ => continue,
            };

            let query = match Message::from_vec(&buf[..len]) {
                Ok(m) => m,
                Err(_) => continue,
            };

            let socket = Arc::clone(&socket);
            let delay_min = delay_min;
            let delay_max = delay_max;
            tokio::spawn(async move {
                // synthetic upstream latency
                let jitter_ms = rand::thread_rng()
                    .gen_range(delay_min.as_millis()..delay_max.as_millis())
                    as u64;
                tokio::time::sleep(Duration::from_millis(jitter_ms)).await;

                let mut resp = Message::new();
                resp.set_id(query.id());
                resp.set_message_type(MessageType::Response);
                resp.set_op_code(OpCode::Query);
                resp.set_response_code(ResponseCode::NoError);
                resp.set_recursion_available(true);

                // Copy query section
                for q in query.queries() {
                    resp.add_query(q.clone());
                }

                // Canned answer: always 93.184.216.34
                if let Some(q) = query.queries().first() {
                    let rdata = RData::A(
                        "93.184.216.34"
                            .parse::<std::net::Ipv4Addr>()
                            .unwrap()
                            .into(),
                    );
                    let record = Record::from_rdata(q.name().clone(), 300, rdata);
                    resp.add_answer(record);
                }

                if let Ok(wire) = resp.to_vec() {
                    let _ = socket.send_to(&wire, src).await;
                }
            });
        }
    });

    addr
}

// ─── query generation ───────────────────────────────────────────────

fn build_query_wire(name: &str, qtype: RecordType, id: u16) -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(id);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(true);
    let mut query = Query::new();
    query.set_name(Name::from_ascii(name).unwrap());
    query.set_query_type(qtype);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    msg.to_vec().unwrap()
}

/// Build the query mix.  Returns (label, wire_bytes) pairs.
fn build_query_mix() -> Vec<(&'static str, Vec<u8>)> {
    let mut mix = Vec::new();
    let mut id: u16 = 0;

    // 20% authoritative
    for name in &[
        "www.cern.ch.",
        "mail.cern.ch.",
        "lhc.cern.ch.",
        "atlas.cern.ch.",
        "cms.cern.ch.",
        "alice.cern.ch.",
        "lhcb.cern.ch.",
        "ns1.cern.ch.",
        "cern.ch.",
        "www.cern.ch.",
    ] {
        mix.push(("auth", build_query_wire(name, RecordType::A, id)));
        id += 1;
    }

    // 70% cache-hit (recursive names that will be pre-warmed)
    for i in 0..35 {
        let name = format!("cached-{i}.example.com.");
        mix.push(("cache", build_query_wire(&name, RecordType::A, id)));
        id += 1;
    }

    // 10% cache-miss (these will actually hit the stub upstream)
    for i in 0..5 {
        let name = format!("miss-{i}.upstream.net.");
        mix.push(("miss", build_query_wire(&name, RecordType::A, id)));
        id += 1;
    }

    mix
}

// ─── latency collection ─────────────────────────────────────────────

struct LatencyBuckets {
    auth: parking_lot::Mutex<Vec<u64>>,
    cache_hit: parking_lot::Mutex<Vec<u64>>,
    cache_miss: parking_lot::Mutex<Vec<u64>>,
    all: parking_lot::Mutex<Vec<u64>>,
}

impl LatencyBuckets {
    fn new(cap: usize) -> Self {
        Self {
            auth: parking_lot::Mutex::new(Vec::with_capacity(cap)),
            cache_hit: parking_lot::Mutex::new(Vec::with_capacity(cap)),
            cache_miss: parking_lot::Mutex::new(Vec::with_capacity(cap)),
            all: parking_lot::Mutex::new(Vec::with_capacity(cap)),
        }
    }

    fn record(&self, kind: &str, nanos: u64) {
        match kind {
            "auth" => self.auth.lock().push(nanos),
            "cache" => self.cache_hit.lock().push(nanos),
            "miss" => self.cache_miss.lock().push(nanos),
            _ => {}
        }
        self.all.lock().push(nanos);
    }

    fn report(&self) {
        fn percentiles(label: &str, v: &parking_lot::Mutex<Vec<u64>>) {
            let mut s = v.lock().clone();
            s.sort_unstable();
            let n = s.len();
            if n == 0 {
                println!("  {label:12} (no samples)");
                return;
            }
            println!(
                "  {label:12} n={n:>6}  p50={:>8.1?}  p95={:>8.1?}  p99={:>8.1?}  max={:>8.1?}",
                Duration::from_nanos(s[n * 50 / 100]),
                Duration::from_nanos(s[n * 95 / 100]),
                Duration::from_nanos(s[n.saturating_sub(1) * 99 / 100]),
                Duration::from_nanos(s[n - 1]),
            );
        }

        percentiles("ALL", &self.all);
        percentiles("auth", &self.auth);
        percentiles("cache-hit", &self.cache_hit);
        percentiles("cache-miss", &self.cache_miss);
    }
}

// ─── load driver ────────────────────────────────────────────────────

async fn run_mixed_load(
    server_addr: SocketAddr,
    num_tasks: usize,
    queries_per_task: usize,
    query_mix: &[(&'static str, Vec<u8>)],
) {
    let total = num_tasks * queries_per_task;
    let buckets = Arc::new(LatencyBuckets::new(total));
    let errors = Arc::new(AtomicU64::new(0));
    let mix = Arc::new(query_mix.to_vec());

    let wall_start = Instant::now();

    let mut handles = Vec::with_capacity(num_tasks);
    for task_id in 0..num_tasks {
        let buckets = Arc::clone(&buckets);
        let errors = Arc::clone(&errors);
        let mix = Arc::clone(&mix);

        handles.push(tokio::spawn(async move {
            let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            sock.connect(server_addr).await.unwrap();
            let mut recv_buf = [0u8; 4096];

            for i in 0..queries_per_task {
                let idx = (task_id * 97 + i) % mix.len(); // pseudo-random spread
                let (kind, pkt) = &mix[idx];

                let start = Instant::now();
                if sock.send(pkt).await.is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                match tokio::time::timeout(Duration::from_secs(5), sock.recv(&mut recv_buf)).await {
                    Ok(Ok(_)) => {
                        buckets.record(kind, start.elapsed().as_nanos() as u64);
                    }
                    _ => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let wall = wall_start.elapsed();
    let ok_count = {
        let a = buckets.all.lock().len();
        a
    };
    let err = errors.load(Ordering::Relaxed);
    let qps = ok_count as f64 / wall.as_secs_f64();

    println!("\n--- {num_tasks} tasks x {queries_per_task} queries (mixed workload) ---");
    println!("  wall time : {wall:.2?}");
    println!("  QPS       : {qps:.0}");
    println!("  errors    : {err}");
    buckets.report();
}

// ─── main ───────────────────────────────────────────────────────────

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        // 1. Start the stub upstream (2–20 ms jitter)
        let stub_stop = Arc::new(AtomicBool::new(false));
        let stub_addr = start_stub_upstream(
            Duration::from_millis(2),
            Duration::from_millis(20),
            Arc::clone(&stub_stop),
        )
        .await;
        println!("stub upstream listening on {stub_addr}");

        // 2. Load the authoritative zone
        let zone = parse_zone_str(ZONE_DATA, &PathBuf::from("cern.ch.zone")).unwrap();
        let mut zones = HashMap::new();
        zones.insert(zone.origin.clone(), zone);
        let store = Arc::new(ZoneStore::new(zones));

        // 3. Build a resolver that forwards everything to the stub
        let mut fwd_zones = HashMap::new();
        fwd_zones.insert(".".to_string(), vec![stub_addr]);

        let recursion_cfg = RecursionConfig {
            enabled: true,
            timeout: "2s".to_string(),
            retries: 1,
            qname_minimization: false,
            forwarding: ForwardingConfig { zones: fwd_zones },
            ..Default::default()
        };
        let cache_cfg = CacheConfig {
            max_entries: 500_000,
            min_ttl: 30,
            max_ttl: 86400,
            negative_ttl: 300,
        };
        let resolver = Arc::new(Resolver::new(&recursion_cfg, &cache_cfg));

        // 4. Pre-warm the cache with the "cache hit" names
        for i in 0..35 {
            let name = Name::from_ascii(format!("cached-{i}.example.com.")).unwrap();
            let _ = resolver.resolve(&name, RecordType::A).await;
        }
        println!("cache warmed with 35 names");

        // 5. Build the full router with auth zone + resolver
        let acl = AclEngine::new(HashMap::new(), "any", "any");
        let router = Arc::new(Router::new(store, Some(resolver), acl));

        // 6. Start the DNS server on localhost
        let cancel = CancellationToken::new();
        let server_addr: SocketAddr = "127.0.0.1:15354".parse().unwrap();
        let server_handles =
            dns_transport::udp::run(&[server_addr], Arc::clone(&router), cancel.clone())
                .await
                .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 7. Build the query mix
        let mix = build_query_mix();
        let auth_pct = mix.iter().filter(|(k, _)| *k == "auth").count() * 100 / mix.len();
        let cache_pct = mix.iter().filter(|(k, _)| *k == "cache").count() * 100 / mix.len();
        let miss_pct = mix.iter().filter(|(k, _)| *k == "miss").count() * 100 / mix.len();
        println!("query mix: {auth_pct}% auth, {cache_pct}% cache-hit, {miss_pct}% cache-miss");

        println!("\n========== mixed-workload throughput benchmark ==========");

        // Warm-up run
        run_mixed_load(server_addr, 4, 500, &mix).await;

        // Scaling runs
        for &tasks in &[1, 4, 16, 64] {
            run_mixed_load(server_addr, tasks, 2_000, &mix).await;
        }

        // 8. Tear down
        cancel.cancel();
        for h in server_handles {
            h.await.unwrap();
        }
        stub_stop.store(true, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_millis(300)).await;
    });
}
