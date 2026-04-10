# CERN DNS Server — Project Specification

## 1. Overview

A modern DNS server written in Rust, designed to replace the existing legacy DNS infrastructure at CERN. The server operates as both an **authoritative nameserver** for CERN-managed zones (cern.ch, open-science.edu, etc.) and a **recursive resolver** for all other queries, serving as the default nameserver on all CERN instances.

### 1.1 Design Principles

- **Separation of concerns**: The DNS server is a pure DNS engine. It does not manage configuration lifecycle. Zone data ingestion, provisioning pipelines (Oracle DB, Kafka, SOAP bridges) are external projects that produce zone files or call a reload API.
- **Zero-downtime reloads**: Configuration changes apply atomically without dropping in-flight queries or requiring process restarts.
- **Observable by default**: Metrics, structured logging, and health endpoints are first-class, not afterthoughts.
- **Defense in depth**: The server must be resilient against cache poisoning, amplification attacks, and malformed input, given its exposure to the full internet DNS hierarchy on the recursive path.

### 1.2 Scale Requirements

| Metric | Target |
|--------|--------|
| Query throughput | ≥ 10,000 queries/sec sustained per instance |
| Managed zones | ~10,000 domains (cern.ch subdomains + other zones) |
| Reload latency | < 1 second for full zone reload |
| Query latency (authoritative) | < 1 ms p99 |
| Query latency (recursive, cached) | < 1 ms p99 |
| Query latency (recursive, cold) | best-effort, bounded by upstream RTT |

---

## 2. Protocol Support

### 2.1 Core DNS

- Full RFC 1035 compliance (DNS message format, standard RRTYPEs).
- EDNS0 (RFC 6891): support for extended message sizes, OPT pseudo-record, and EDNS options including Client Subnet (ECS, RFC 7871) as a pass-through.
- DNS over UDP (primary transport).
- DNS over TCP (RFC 7766): full support, not just fallback for truncated responses. Persistent TCP connections with configurable idle timeouts.

### 2.2 Encrypted Transports

- DNS over TLS (DoT, RFC 7858) on port 853.
- DNS over HTTPS (DoH, RFC 8484) on port 443, supporting both `application/dns-message` (wire format) and `application/dns-json` (JSON API).
- TLS certificate management via configurable paths (integration with CERN's CA infrastructure or Let's Encrypt).

### 2.3 DNSSEC

- **Authoritative signing**: automatic zone signing with configurable key management (ZSK/KSK rotation schedules, algorithm selection — at minimum ECDSAP256SHA256 and ED25519).
- **Recursive validation**: full DNSSEC validation on recursive responses, with configurable trust anchors (auto-updated root KSK via RFC 5011).
- NSEC and NSEC3 support for authenticated denial of existence.

---

## 3. Architecture

### 3.1 High-Level Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Network Layer                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────────┐  │
│  │ UDP Listener │ TCP Listener │ DoT Listener │ DoH (HTTP/2)  │  │
│  └─────┬────┘  └─────┬────┘  └─────┬────┘  └──────┬─────────┘  │
│        └──────────────┴──────────────┴──────────────┘            │
│                              │                                   │
│                     ┌────────▼────────┐                          │
│                     │  Query Router   │                          │
│                     └───┬─────────┬───┘                          │
│                         │         │                              │
│              ┌──────────▼──┐  ┌───▼──────────────┐              │
│              │ Authoritative│  │ Recursive Resolver│              │
│              │   Engine     │  │                   │              │
│              │              │  │  ┌─────────────┐ │              │
│              │  ┌────────┐  │  │  │  Cache       │ │              │
│              │  │Zone Store│ │  │  └─────────────┘ │              │
│              │  └────────┘  │  │  ┌─────────────┐ │              │
│              │              │  │  │ Upstream Pool│ │              │
│              └──────────────┘  │  └─────────────┘ │              │
│                                └──────────────────┘              │
│                              │                                   │
│                     ┌────────▼────────┐                          │
│                     │  Response Builder│                          │
│                     │  (DNSSEC, EDNS0) │                          │
│                     └─────────────────┘                          │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐ │
│  │ Metrics/Stats │  │ Reload API   │  │ Health / Readiness     │ │
│  └──────────────┘  └──────────────┘  └────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Query Router

The query router is the central dispatch point. For every incoming query:

1. Parse and validate the DNS message. Drop malformed packets.
2. Apply access control (ACLs — see §5).
3. Check if the query name falls within any locally authoritative zone.
   - **Yes** → dispatch to the Authoritative Engine.
   - **No** → dispatch to the Recursive Resolver (if ACL permits recursion for the source).
4. Pass the response through the Response Builder for EDNS0 handling, truncation, and optional DNSSEC signing.

### 3.3 Authoritative Engine

The authoritative engine serves answers from the Zone Store, an in-memory data structure holding all managed zones.

**Zone Store design**:

- Zones are stored in a concurrent map keyed by zone name, where each zone contains a trie or sorted map of domain names to record sets.
- The active zone data is wrapped in `Arc<ZoneStore>`. Reads are lock-free via atomic reference counting.
- On reload, a new `ZoneStore` is built in the background and swapped atomically via `Arc::swap`. In-flight queries on the old `Arc` complete naturally; the old data is dropped when the last reference is released.

**Query processing**:

- Exact match, wildcard expansion (RFC 4592), CNAME chasing (within the same zone), DNAME (RFC 6672).
- Delegation responses with NS + glue records for subzones.
- Negative responses: NXDOMAIN with SOA in authority section, NODATA with SOA.
- DNSSEC: on-the-fly signing or pre-signed RRSIG records served from the zone store.

### 3.4 Recursive Resolver

The recursive resolver handles queries for domains outside the authoritative zones.

**Resolution process**:

- Standard iterative resolution starting from the root hints, walking referrals down the delegation chain.
- Query name minimization (RFC 9156) to limit information leakage to upstream authoritative servers.
- Concurrent resolution: multiple in-flight recursive queries are deduplicated — if 100 clients ask for the same name simultaneously, only one upstream walk is performed and all waiters receive the result.

**Cache**:

- TTL-respecting cache with maximum TTL cap (configurable, default 86400s).
- Minimum TTL floor (configurable, default 30s) to prevent excessive upstream load from TTL 0 records.
- Negative caching per RFC 2308 (NXDOMAIN and NODATA responses cached based on SOA minimum TTL).
- Cache size limit with LRU or ARC eviction policy.
- Manual cache flush API (full flush or per-name/per-zone).

**Security hardening**:

- Source port randomization (RFC 5452).
- 0x20 mixed-case query encoding for additional entropy.
- Response validation: reject mismatched transaction IDs, unexpected answers, glue outside bailiwick.
- DNSSEC validation of recursive responses (see §2.4).

### 3.5 Upstream Connection Pool

For recursive resolution, the server maintains a pool of connections to upstream authoritative servers:

- UDP with configurable retry and timeout (default: 2s timeout, 2 retries).
- TCP fallback on truncation.
- Connection reuse for TCP where supported.
- Optional forwarding mode: instead of full recursion, forward to configured upstream resolvers (e.g., for split-horizon or policy reasons). This is a configuration option per zone or as a global fallback.

---

## 4. Zone Data Management

### 4.1 Zone File Format

The server reads standard RFC 1035 zone files as the primary input format. This ensures compatibility with existing tooling and any upstream provisioning system.

Supported record types (minimum):

A, AAAA, CNAME, MX, NS, PTR, SOA, SRV, TXT, CAA, DNAME, NAPTR, SSHFP, TLSA, LOC, HINFO, RP.

Additional types can be added by extending the record parser without architectural changes.

### 4.2 Zone Loading and Reloading

**Startup**: All zone files from the configured zone directory are loaded into the Zone Store. The server does not begin accepting queries until initial zone loading is complete (readiness probe reflects this).

**Reload triggers** (any of):

- HTTP API call: `POST /api/v1/reload` (reload all zones) or `POST /api/v1/reload/{zone}` (single zone).
- Unix signal: `SIGHUP` triggers a full reload.
- File watcher: inotify-based watcher on the zone directory (optional, configurable).

**Reload semantics**:

- Zones are parsed and validated in a background task.
- If parsing fails for a zone, the old version is retained and an error is logged/exposed via metrics. The reload does not fail atomically across all zones — healthy zones are updated, broken zones keep their previous version.
- The swap is atomic from the query path's perspective: a query either sees the old data or the new data, never a partial state.

### 4.3 Reverse DNS

Full support for in-addr.arpa (IPv4) and ip6.arpa (IPv6) reverse zones, loaded and served identically to forward zones.

---

## 5. Access Control

### 5.1 ACL Configuration

Access control lists define per-source-network policies:

- **allow-recursion**: which source networks may use the recursive resolver. Default: CERN internal ranges only.
- **allow-query**: which networks may query authoritative zones. Default: any (public authoritative zones).
- **rate-limit**: per-source-IP or per-subnet rate limits (see §6.2).

ACLs are defined in the server configuration file and can reference named network groups for readability.

### 5.2 Example

```yaml
acls:
  cern-internal:
    - 128.141.0.0/16
    - 128.142.0.0/16
    - 137.138.0.0/16
    - 188.184.0.0/15
    - 192.91.242.0/24
    - 192.16.155.0/24
    - 2001:1458::/32
    - 2001:1459::/32

policy:
  allow-recursion: cern-internal
  allow-query: any
```

---

## 6. Security

### 6.1 Cache Poisoning Mitigations

- Source port randomization on all outgoing recursive queries.
- 0x20 mixed-case encoding: the query name case is randomized and verified in the response.
- Transaction ID randomization (full 16-bit entropy).
- Bailiwick checking: glue records outside the delegated zone are discarded.
- DNSSEC validation: the strongest defense — cryptographically verified responses.

### 6.2 Response Rate Limiting (RRL)

To prevent the server from being used as a DNS amplification vector:

- Rate limiting on identical responses per source IP/prefix (configurable prefix length, default /24 for IPv4, /48 for IPv6).
- Slip ratio: configurable fraction of rate-limited responses that are sent as truncated (TC=1) instead of being dropped entirely, to allow legitimate clients to retry over TCP.
- Separate rate limit buckets for: NODATA, NXDOMAIN, referrals, and positive answers.

### 6.3 Input Validation

- Maximum DNS message size enforcement.
- Label length validation (max 63 bytes per label, 253 bytes total).
- Compression pointer loop detection.
- Query count limits (reject messages with QDCOUNT != 1).
- TCP connection limits per source IP.

---

## 7. Observability

### 7.1 Metrics (Prometheus)

Exposed via HTTP endpoint (`/metrics`, configurable port).

**Query metrics**:

- `dns_queries_total{transport, qtype, zone, response_code}` — counter of queries by transport (udp/tcp/dot/doh), query type, zone (or "recursive"), and response code (NOERROR, NXDOMAIN, SERVFAIL, REFUSED).
- `dns_query_duration_seconds{path}` — histogram of query latency, labeled by path (authoritative vs recursive).
- `dns_recursive_upstream_queries_total{server}` — counter of outgoing recursive queries per upstream server.
- `dns_recursive_upstream_duration_seconds{server}` — histogram of upstream query latency.

**Cache metrics**:

- `dns_cache_size` — gauge of current cache entries.
- `dns_cache_hits_total` / `dns_cache_misses_total` — counters.
- `dns_cache_evictions_total` — counter.

**Zone metrics**:

- `dns_zone_records_total{zone}` — gauge of records per zone.
- `dns_zone_reload_timestamp{zone}` — gauge of last successful reload time.
- `dns_zone_reload_errors_total{zone}` — counter of failed reloads.

**System metrics**:

- `dns_rrl_dropped_total` / `dns_rrl_truncated_total` — rate limiting counters.
- `dns_tcp_connections_active` — gauge.
- `dns_inflight_recursive_queries` — gauge.

### 7.2 Structured Logging

All logs emitted in JSON format (compatible with ELK, Loki, or any structured log pipeline). Configurable log level (error, warn, info, debug, trace).

Key log events: zone reload (success/failure), DNSSEC key rotation events, rate limiting activations, SERVFAIL on recursive resolution with cause, ACL denials.

### 7.3 Health Endpoints

- `GET /health/live` — process is alive (always 200 after startup).
- `GET /health/ready` — zones are loaded, server is accepting queries (503 during initial load or if critical zones fail to load).

---

## 8. Configuration

### 8.1 Configuration File

TOML format. Example:

```toml
[server]
listen_udp = ["0.0.0.0:53", "[::]:53"]
listen_tcp = ["0.0.0.0:53", "[::]:53"]
listen_dot = ["0.0.0.0:853", "[::]:853"]
listen_doh = ["0.0.0.0:443", "[::]:443"]
listen_http = "127.0.0.1:9153"  # metrics + API + health, internal only
workers = 0  # 0 = auto-detect CPU count

[tls]
cert_path = "/etc/dns/tls/cert.pem"
key_path = "/etc/dns/tls/key.pem"

[zones]
directory = "/etc/dns/zones"
watch = true  # inotify-based auto-reload

[recursion]
enabled = true
root_hints = "/etc/dns/root.hints"
max_depth = 30  # maximum referral chain depth
timeout = "2s"
retries = 2
qname_minimization = true

[recursion.forwarding]
# Optional: forward specific zones instead of full recursion
# "." = forward everything (forwarder mode)
# zones = { "example.com" = ["10.0.0.1:53", "10.0.0.2:53"] }

[cache]
max_entries = 1_000_000
max_ttl = 86400
min_ttl = 30
negative_ttl = 300

[dnssec]
enable_signing = true
enable_validation = true
key_directory = "/etc/dns/keys"
auto_rotate = true

[dnssec.algorithms]
zsk = "ECDSAP256SHA256"
ksk = "ECDSAP256SHA256"

[rrl]
enabled = true
responses_per_second = 5
slip = 2  # 1 in N rate-limited responses sent as TC=1
ipv4_prefix_length = 24
ipv6_prefix_length = 48

[logging]
level = "info"
format = "json"
```

### 8.2 Command-Line Interface

```
cern-dns --config /etc/dns/config.toml
cern-dns --check-config /etc/dns/config.toml   # validate without starting
cern-dns --check-zone /etc/dns/zones/cern.ch    # validate a zone file
cern-dns --version
```

---

## 9. Deployment

### 9.1 Build Artifact

Single statically-linked binary (musl target). No runtime dependencies beyond libc. Container image based on `scratch` or `distroless`.

### 9.2 Systemd Integration

- Type=notify for readiness signaling.
- Capabilities: CAP_NET_BIND_SERVICE (bind to port 53/443/853 without root).
- Sandboxing: ProtectSystem=strict, PrivateTmp=yes, NoNewPrivileges=yes.

### 9.3 Instance Topology

Multiple instances behind an anycast IP or load balancer. Each instance is fully independent — there is no primary/secondary relationship and no DNS-native replication between instances. Zone consistency is achieved by the external config management layer pushing identical zone files to all instances, similar to how cloud DNS providers (e.g., AWS Route 53) distribute data internally.

---

## 10. Testing Strategy

### 10.1 Unit Tests

- DNS message parsing and serialization (round-trip fuzz testing).
- Zone file parsing (valid and malformed inputs).
- Cache insertion, lookup, TTL expiry, eviction.
- ACL matching.
- DNSSEC signature generation and validation.

### 10.2 Integration Tests

- Full query lifecycle: client → server → response, for all transports (UDP, TCP, DoT, DoH).
- Recursive resolution against a mock upstream hierarchy.
- Reload semantics: verify no queries are dropped during zone swap.
- RRL behavior under synthetic load.

### 10.3 Conformance Tests

- Run against the DNS compliance test suites (e.g., `dnscheck`, BIND's test framework).
- RFC compliance spot checks for edge cases: wildcard CNAME, empty non-terminal, DNAME + CNAME interaction, oversized UDP truncation.

### 10.4 Performance Tests

- Sustained throughput benchmark: target ≥ 50,000 qps per instance on commodity hardware (well above the 10k requirement, leaving headroom).
- Latency distribution under load (p50, p95, p99, p999).
- Zone reload under load: measure query latency impact during reload.
- Memory usage with full cache and all zones loaded.

---

## 11. Migration Path

### 11.1 Phase 1 — Shadow Mode

Deploy alongside existing DNS infrastructure. Mirror a copy of production queries to the new server and compare responses. Log discrepancies. No client-facing traffic.

### 11.2 Phase 2 — Canary

Route a small percentage of queries (e.g., a single subnet) to the new server. Monitor metrics and compare with production.

### 11.3 Phase 3 — Gradual Rollout

Increase traffic percentage. Keep the old infrastructure on standby for instant rollback.

### 11.4 Phase 4 — Full Cutover

All traffic served by the new server. Decommission legacy infrastructure.

---

## 12. Out of Scope

The following are explicitly **not** part of this project and belong to separate configuration management efforts:

- Web UI or management console for DNS records.
- Integration with Oracle DB, Kafka, SOAP, or any provisioning pipeline.
- IPAM (IP Address Management).
- DHCP integration.
- DNS-based load balancing logic (GSLB). The server serves what is in the zone files; traffic steering logic belongs in the provisioning layer that generates those zone files.
- Recursive-only features like DNS-based ad blocking or content filtering.
- Zone transfers (AXFR/IXFR/NOTIFY/TSIG). Zone data replication is handled by the external config management layer pushing zone files to all instances, not by DNS-native transfer protocols.

---

## 13. Code Structure

### 13.1 Workspace Layout

The project uses a Cargo workspace to enforce separation of concerns at the compilation level. Each crate has a single responsibility and depends only on what it needs. A developer looking at the top-level directory should immediately understand where to find any piece of functionality.

```
cern-dns/
├── Cargo.toml                  # workspace root
├── Cargo.lock
├── README.md
├── config/                     # example configuration files
│   ├── config.toml
│   └── zones/
│       └── example.cern.ch.zone
│
├── crates/
│   ├── dns-server/             # binary crate — the entry point
│   │   └── src/
│   │       ├── main.rs         # CLI parsing, signal handling, bootstrap
│   │       └── shutdown.rs     # graceful shutdown orchestration
│   │
│   ├── dns-protocol/           # DNS wire format: parsing, serialization, types
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── message.rs      # DNS message (header, question, answer, authority, additional)
│   │       ├── name.rs         # domain name encoding/decoding, compression
│   │       ├── record.rs       # RData types (A, AAAA, CNAME, MX, SOA, SRV, TXT, ...)
│   │       ├── edns.rs         # EDNS0 OPT record, options
│   │       ├── opcode.rs       # opcodes, response codes
│   │       └── serialize.rs    # wire format read/write traits
│   │
│   ├── dns-authority/          # authoritative engine: zone store, lookups
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── zone.rs         # single zone: records, SOA, lookup logic
│   │       ├── zone_store.rs   # concurrent map of all zones, Arc-swap reload
│   │       ├── loader.rs       # zone file parser (RFC 1035 format)
│   │       ├── lookup.rs       # query matching: exact, wildcard, CNAME, DNAME, delegation
│   │       └── negative.rs     # NXDOMAIN / NODATA response construction
│   │
│   ├── dns-resolver/           # recursive resolver
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── resolver.rs     # iterative resolution logic, referral walking
│   │       ├── cache.rs        # TTL-aware cache, LRU eviction, negative caching
│   │       ├── dedup.rs        # in-flight query deduplication
│   │       ├── upstream.rs     # connection pool to upstream servers (UDP/TCP)
│   │       ├── qname_min.rs    # query name minimization (RFC 9156)
│   │       └── validator.rs    # DNSSEC validation of recursive responses
│   │
│   ├── dns-dnssec/             # DNSSEC signing and validation primitives
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── signer.rs       # zone signing (ZSK/KSK), RRSIG generation
│   │       ├── verifier.rs     # signature verification, chain-of-trust walking
│   │       ├── keys.rs         # key management, rotation, trust anchors
│   │       └── nsec.rs         # NSEC / NSEC3 generation and validation
│   │
│   ├── dns-transport/          # network listeners and connection handling
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── udp.rs          # UDP listener, recv/send loop
│   │       ├── tcp.rs          # TCP listener, connection state, idle timeout
│   │       ├── dot.rs          # DNS-over-TLS (wraps tcp with rustls)
│   │       ├── doh.rs          # DNS-over-HTTPS (axum handler)
│   │       └── rate_limit.rs   # per-transport connection limits
│   │
│   ├── dns-router/             # query routing and response building
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs       # dispatch: authoritative vs recursive
│   │       ├── acl.rs          # access control list matching
│   │       ├── response.rs     # response assembly, truncation, EDNS0 fixup
│   │       └── rrl.rs          # response rate limiting
│   │
│   ├── dns-api/                # HTTP management API and health/metrics
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── routes.rs       # axum routes: /reload, /cache/flush, etc.
│   │       ├── health.rs       # /health/live, /health/ready
│   │       └── metrics.rs      # Prometheus /metrics endpoint
│   │
│   └── dns-config/             # configuration parsing and validation
│       └── src/
│           ├── lib.rs
│           ├── config.rs       # TOML deserialization, defaults
│           └── validation.rs   # semantic validation (port conflicts, path existence, etc.)
│
├── tests/                      # workspace-level integration tests
│   ├── auth_test.rs            # authoritative query lifecycle
│   ├── recursive_test.rs       # recursive resolution against mock hierarchy
│   ├── transport_test.rs       # UDP, TCP, DoT, DoH end-to-end
│   ├── reload_test.rs          # zone reload under load
│   ├── acl_test.rs             # access control enforcement
│   ├── rrl_test.rs             # rate limiting behavior
│   └── dnssec_test.rs          # signing and validation end-to-end
│
├── benches/                    # criterion benchmarks
│   ├── parsing.rs              # message parse/serialize throughput
│   ├── lookup.rs               # zone lookup latency
│   └── cache.rs                # cache read/write throughput
│
└── tools/                      # development and operational utilities
    ├── zone-lint/              # standalone zone file validator
    └── query-replay/           # replay pcap/query logs against the server
```

### 13.2 Design Principles for the Codebase

**Dependency direction**: crates depend downward, never upward or circularly. `dns-server` depends on everything. `dns-router` depends on `dns-authority`, `dns-resolver`, `dns-transport`, and `dns-protocol`. The leaf crates (`dns-protocol`, `dns-config`) depend on nothing internal. This enforces layering and makes each crate independently testable.

```
dns-server
   └── dns-router
          ├── dns-authority ──→ dns-protocol
          ├── dns-resolver ──→ dns-protocol, dns-dnssec
          ├── dns-transport ──→ dns-protocol
          └── dns-api
   └── dns-config
   └── dns-dnssec ──→ dns-protocol
```

**Traits as boundaries**: each major component exposes a trait that defines its contract. The router depends on `trait AuthoritativeEngine` and `trait RecursiveResolver`, not on concrete types. This enables unit testing with mocks and allows swapping implementations without touching callers.

```rust
// dns-authority/src/lib.rs
pub trait AuthoritativeEngine: Send + Sync {
    fn lookup(&self, query: &Query) -> Option<AuthResponse>;
    fn is_authoritative_for(&self, name: &Name) -> bool;
}

// dns-resolver/src/lib.rs
pub trait RecursiveResolver: Send + Sync {
    async fn resolve(&self, query: &Query) -> Result<ResolveResponse, ResolveError>;
}
```

**Error handling**: each crate defines its own error enum using `thiserror`. Errors from lower crates are wrapped, not leaked. The transport layer never sees a `ZoneParseError`; it sees a `ServerError::Servfail` with the cause attached for logging.

**No business logic in `main.rs`**: the binary crate is pure wiring. It reads config, constructs the components, wires them together, starts the listeners, and waits for shutdown. If `main.rs` grows beyond ~100 lines, logic is being put in the wrong place.

**Configuration flows one way**: `dns-config` parses the TOML into typed structs. Each crate receives only the config slice it needs (e.g., `dns-resolver` receives `RecursionConfig`, not the entire server config). No crate reads files or environment variables directly.

**Testing at every level**: each crate has its own `#[cfg(test)]` unit tests next to the code. Integration tests in the workspace `tests/` directory spin up the full server (or relevant subsystems) and test through the public interfaces. Benchmarks in `benches/` use criterion for reproducible performance measurement.

**Unsafe code policy**: no `unsafe` blocks unless strictly necessary for performance-critical paths (e.g., zero-copy packet parsing), and any such usage must be documented with a safety comment explaining the invariants.

---

## 14. Technology Choices

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Memory safety, no GC, predictable latency, strong concurrency |
| Async runtime | tokio | De facto standard, mature, multi-threaded work-stealing |
| DNS wire format | hickory-dns (trust-dns) proto crate | Battle-tested parser, avoid reinventing the wire format |
| HTTP (DoH + API) | hyper / axum | Lightweight, async-native, widely used |
| TLS | rustls | Pure Rust, no OpenSSL dependency, audited |
| Metrics | prometheus-client | Official Rust Prometheus client |
| Logging | tracing + tracing-subscriber | Structured, async-aware, JSON output |
| Config | toml (serde) | Simple, readable, Rust-native |
| Zone parsing | Custom or zone-file crate | Standard RFC 1035 format, straightforward to parse |
