# CERN DNS Server

A high-performance DNS server written in Rust, designed to serve as both an **authoritative nameserver** and a **recursive resolver**. Built for CERN's infrastructure, it handles authoritative zones (cern.ch, etc.) and resolves all other queries recursively.

## Features

- **Authoritative serving** -- zone file loading, exact/wildcard/CNAME matching, delegation, NXDOMAIN/NODATA
- **Recursive resolution** -- iterative resolution from root hints, query deduplication, QNAME minimization (RFC 9156)
- **Caching** -- TTL-aware with min/max TTL clamping, negative caching (RFC 2308), LRU eviction
- **Forwarding** -- per-zone or global forwarding to upstream resolvers
- **Transports** -- UDP, TCP, DNS-over-TLS (DoT), DNS-over-HTTPS (DoH)
- **DNSSEC** -- zone signing (ECDSAP256SHA256, ED25519), NSEC chains, response validation
- **Security** -- Response Rate Limiting (RRL), ACLs, source port randomization, 0x20 encoding, bailiwick checks
- **Observability** -- Prometheus metrics, structured JSON logging, health/readiness endpoints
- **Zero-downtime reloads** -- atomic zone swaps via `Arc`; in-flight queries are never dropped
- **Management API** -- HTTP endpoints for reload, cache flush, zone listing

## Performance

Measured on a MacBook Pro (M-series, 4 tokio worker threads):

| Workload | Concurrency | QPS | p50 | p99 |
|---|---|---|---|---|
| Authoritative only | 64 tasks | ~95,000 | 155 us | 311 us |
| Mixed (20% auth, 70% cache, 10% miss) | 64 tasks | ~104,000 | 617 us | 1.0 ms |

Micro-benchmarks: authoritative lookup ~5 us, cache get ~371 ns, message parse ~151 ns.

## Quick start

```bash
# Build
cargo build --release

# Run with example config
./target/release/cern-dns --config config/config.toml

# Query it
dig @127.0.0.1 -p 5353 example.cern.ch A
```

## Documentation

- **[Installation guide](.docs/installation.md)** -- building from source, dependencies, binary targets
- **[Operations guide](.docs/operations.md)** -- configuration reference, zone management, monitoring, deployment
- **[Specification](.docs/dns-spec.md)** -- full project specification and architecture

## Project structure

```
crates/
  dns-config/       Configuration parsing and validation
  dns-protocol/     DNS wire format wrappers (over hickory-proto)
  dns-authority/    Authoritative engine: zone store, lookups
  dns-resolver/     Recursive resolver with cache and dedup
  dns-dnssec/       DNSSEC signing, verification, NSEC
  dns-transport/    Network listeners: UDP, TCP, DoT, DoH
  dns-router/       Query routing, ACLs, RRL, response building
  dns-api/          HTTP management API, health, metrics
  dns-server/       Binary crate -- CLI, bootstrap, wiring
config/
  config.toml       Example configuration
  zones/            Example zone files
```

## Development

```bash
# Run all tests (111 tests)
cargo test --workspace

# Run clippy
cargo clippy --workspace --all-targets

# Run criterion benchmarks
cargo bench -p dns-server --bench parsing
cargo bench -p dns-server --bench lookup
cargo bench -p dns-server --bench cache

# Run throughput benchmarks
cargo bench -p dns-server --bench throughput
cargo bench -p dns-server --bench throughput_mixed
```

## CI

The GitHub Actions pipeline (`.github/workflows/ci.yml`) runs on every push and PR:

- **check** -- `cargo check` + `cargo clippy` with `-D warnings`
- **fmt** -- `cargo fmt --check`
- **test** -- `cargo test --workspace`
- **bench** (PRs only) -- runs criterion benchmarks and compares against the `main` baseline using [github-action-benchmark](https://github.com/benchmark-action/github-action-benchmark). PRs that regress by more than 20% will fail. Benchmark history is stored in the `gh-pages` branch.

## License

MIT
