//! Concurrent throughput benchmark.
//!
//! Boots the real DNS server on a localhost UDP port, then fires queries
//! from multiple tokio tasks across the thread pool. Reports aggregate QPS
//! and latency distribution (p50 / p95 / p99 / max).

use dns_authority::loader::parse_zone_str;
use dns_authority::ZoneStore;
use dns_router::acl::AclEngine;
use dns_router::router::Router;
use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{DNSClass, Name, RecordType};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

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
app IN  A   192.0.2.50
api IN  A   192.0.2.60
cdn IN  A   192.0.2.70
*.wild IN A 192.0.2.250
"#;

fn build_query_wire(name: &str, qtype: RecordType, id: u16) -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(id);
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

/// A single latency sample in nanoseconds.
struct LatencyCollector {
    samples: parking_lot::Mutex<Vec<u64>>,
}

impl LatencyCollector {
    fn new(capacity: usize) -> Self {
        Self {
            samples: parking_lot::Mutex::new(Vec::with_capacity(capacity)),
        }
    }

    fn record(&self, nanos: u64) {
        self.samples.lock().push(nanos);
    }

    fn report(&self) -> LatencyReport {
        let mut samples = self.samples.lock().clone();
        samples.sort_unstable();
        let n = samples.len();
        if n == 0 {
            return LatencyReport {
                count: 0,
                p50: Duration::ZERO,
                p95: Duration::ZERO,
                p99: Duration::ZERO,
                max: Duration::ZERO,
            };
        }
        LatencyReport {
            count: n,
            p50: Duration::from_nanos(samples[n * 50 / 100]),
            p95: Duration::from_nanos(samples[n * 95 / 100]),
            p99: Duration::from_nanos(samples[n * 99 / 100]),
            max: Duration::from_nanos(samples[n - 1]),
        }
    }
}

struct LatencyReport {
    count: usize,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
}

impl std::fmt::Display for LatencyReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  responses : {}\n  p50       : {:.1?}\n  p95       : {:.1?}\n  p99       : {:.1?}\n  max       : {:.1?}",
            self.count, self.p50, self.p95, self.p99, self.max
        )
    }
}

async fn run_load_test(
    server_addr: SocketAddr,
    num_tasks: usize,
    queries_per_task: usize,
    query_names: &[&str],
) {
    let total_queries = num_tasks * queries_per_task;
    let latencies = Arc::new(LatencyCollector::new(total_queries));
    let errors = Arc::new(AtomicU64::new(0));

    // Pre-build query packets (one per query name, cycled by workers)
    let packets: Vec<Vec<u8>> = query_names
        .iter()
        .enumerate()
        .map(|(i, name)| build_query_wire(name, RecordType::A, i as u16))
        .collect();
    let packets = Arc::new(packets);

    let wall_start = Instant::now();

    let mut handles = Vec::with_capacity(num_tasks);
    for task_id in 0..num_tasks {
        let latencies = Arc::clone(&latencies);
        let errors = Arc::clone(&errors);
        let packets = Arc::clone(&packets);

        handles.push(tokio::spawn(async move {
            // Each task gets its own socket (different source port)
            let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            sock.connect(server_addr).await.unwrap();
            let mut recv_buf = [0u8; 4096];

            for i in 0..queries_per_task {
                let pkt = &packets[(task_id + i) % packets.len()];
                let start = Instant::now();
                if sock.send(pkt).await.is_err() {
                    errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                match tokio::time::timeout(Duration::from_secs(2), sock.recv(&mut recv_buf)).await {
                    Ok(Ok(_len)) => {
                        latencies.record(start.elapsed().as_nanos() as u64);
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

    let wall_elapsed = wall_start.elapsed();
    let report = latencies.report();
    let err_count = errors.load(Ordering::Relaxed);
    let qps = report.count as f64 / wall_elapsed.as_secs_f64();

    println!("\n--- {num_tasks} tasks x {queries_per_task} queries ---");
    println!("  wall time : {wall_elapsed:.2?}");
    println!("  QPS       : {qps:.0}");
    println!("  errors    : {err_count}");
    println!("{report}");
}

fn main() {
    // Build the multi-threaded tokio runtime explicitly so we control worker count
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        // ── stand up the server ──────────────────────────────────────
        let zone =
            parse_zone_str(ZONE_DATA, &std::path::PathBuf::from("example.com.zone")).unwrap();
        let mut zones = HashMap::new();
        zones.insert(zone.origin.clone(), zone);
        let store = Arc::new(ZoneStore::new(zones));
        let acl = AclEngine::new(HashMap::new(), "any", "any");
        let router = Arc::new(Router::new(store, None, acl));
        let cancel = CancellationToken::new();

        // Bind to port 0 so the OS picks a free port
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server_handles = dns_transport::udp::run(&[addr], Arc::clone(&router), cancel.clone())
            .await
            .unwrap();

        // Discover which port the OS assigned
        // We need to read it from the socket before the task takes ownership.
        // Re-bind on a known port instead:
        cancel.cancel(); // stop the ephemeral one
        for h in server_handles {
            h.await.unwrap();
        }

        let cancel = CancellationToken::new();
        let server_addr: SocketAddr = "127.0.0.1:15353".parse().unwrap();
        let server_handles =
            dns_transport::udp::run(&[server_addr], Arc::clone(&router), cancel.clone())
                .await
                .unwrap();

        // Give the listener a moment to be ready
        tokio::time::sleep(Duration::from_millis(50)).await;

        // ── query mix ────────────────────────────────────────────────
        let query_names: Vec<&str> = vec![
            "www.example.com.",
            "mail.example.com.",
            "app.example.com.",
            "api.example.com.",
            "cdn.example.com.",
            "nonexistent.example.com.", // NXDOMAIN
            "random.wild.example.com.", // wildcard
        ];

        println!("========== concurrent throughput benchmark ==========");

        // Warm-up
        run_load_test(server_addr, 4, 1_000, &query_names).await;

        // Scaling: 1 → 4 → 16 → 64 concurrent tasks
        for &tasks in &[1, 4, 16, 64] {
            run_load_test(server_addr, tasks, 5_000, &query_names).await;
        }

        // ── tear down ────────────────────────────────────────────────
        cancel.cancel();
        for h in server_handles {
            h.await.unwrap();
        }
    });
}
