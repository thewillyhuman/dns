# Operations Guide

## Running the server

```bash
cern-dns --config /etc/dns/config.toml
```

The server loads all zone files, starts listeners, and begins accepting queries. It does not respond to DNS queries until zone loading is complete (the `/health/ready` endpoint returns 503 during startup).

## Configuration reference

The server is configured via a single TOML file. All fields have sensible defaults.

### `[server]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `listen_udp` | list of strings | `["0.0.0.0:53"]` | UDP listener addresses |
| `listen_tcp` | list of strings | `["0.0.0.0:53"]` | TCP listener addresses |
| `listen_dot` | list of strings | `[]` | DNS-over-TLS listener addresses (port 853) |
| `listen_doh` | list of strings | `[]` | DNS-over-HTTPS listener addresses |
| `listen_http` | string | `"127.0.0.1:9153"` | Management API / metrics / health endpoint |
| `workers` | integer | `0` | Tokio worker threads. `0` = auto-detect CPU count |

### `[tls]`

Required only if `listen_dot` or `listen_doh` are configured.

| Key | Type | Description |
|-----|------|-------------|
| `cert_path` | string | Path to PEM certificate chain |
| `key_path` | string | Path to PEM private key |

### `[zones]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `directory` | string | `"config/zones"` | Directory containing RFC 1035 zone files |
| `watch` | bool | `false` | Enable inotify-based auto-reload |

Zone files must be named `<zone-name>.zone` (e.g., `cern.ch.zone`). They follow standard RFC 1035 format with `$ORIGIN` and `$TTL` directives.

### `[recursion]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `true` | Enable recursive resolution |
| `max_depth` | integer | `30` | Maximum referral chain depth |
| `timeout` | string | `"2s"` | Per-upstream query timeout |
| `retries` | integer | `2` | Retry count per upstream |
| `qname_minimization` | bool | `true` | RFC 9156 QNAME minimization |

### `[recursion.forwarding]`

Optional. When configured, queries matching the specified zones are forwarded to the listed servers instead of performing full iterative resolution.

```toml
[recursion.forwarding.zones]
"." = ["8.8.8.8:53", "1.1.1.1:53"]                    # Forward everything
"internal.corp." = ["10.0.0.1:53", "10.0.0.2:53"]     # Forward a specific zone
```

Use `"."` as a catch-all to run the server in pure forwarder mode.

### `[cache]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_entries` | integer | `1000000` | Maximum cache entries (LRU eviction) |
| `max_ttl` | integer | `86400` | Maximum TTL in seconds (clamps high TTLs) |
| `min_ttl` | integer | `30` | Minimum TTL in seconds (prevents excessive upstream load) |
| `negative_ttl` | integer | `300` | TTL for negative cache entries (NXDOMAIN/NODATA) |

### `[dnssec]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enable_signing` | bool | `false` | Sign authoritative zones |
| `enable_validation` | bool | `false` | Validate recursive responses |
| `key_directory` | string | `""` | Directory for DNSSEC keys |

### `[rrl]`

Response Rate Limiting protects against DNS amplification attacks.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable RRL |
| `responses_per_second` | integer | `5` | Max identical responses per source prefix per second |
| `slip` | integer | `2` | 1 in N rate-limited responses sent as TC=1 (rest dropped) |
| `ipv4_prefix_length` | integer | `24` | Group IPv4 sources by /24 |
| `ipv6_prefix_length` | integer | `48` | Group IPv6 sources by /48 |

### `[logging]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `level` | string | `"info"` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `format` | string | `"json"` | Output format: `json` or `text` |

### `[policy]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `allow_query` | string | `"any"` | Networks allowed to query. `"any"` or a named ACL group |
| `allow_recursion` | string | `"any"` | Networks allowed to use recursion |

## Zone management

### Adding a zone

1. Create a zone file in the zones directory:

```
; config/zones/example.com.zone
$ORIGIN example.com.
$TTL 3600
@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.example.com.
    IN  A   192.0.2.1
ns1 IN  A   192.0.2.10
www IN  A   192.0.2.100
```

2. Trigger a reload:

```bash
# Via HTTP API
curl -X POST http://127.0.0.1:9153/api/v1/reload

# Via signal
kill -HUP $(pidof cern-dns)
```

### Reloading zones

Reloads are atomic and zero-downtime. In-flight queries complete against the old data; new queries see the updated zones. If a zone file has parse errors, that zone keeps its previous version and an error is logged.

```bash
# Reload all zones
curl -X POST http://127.0.0.1:9153/api/v1/reload

# List loaded zones
curl http://127.0.0.1:9153/api/v1/zones
```

## Cache management

```bash
# Flush entire cache
curl -X POST http://127.0.0.1:9153/api/v1/cache/flush

# Flush a specific name
curl -X POST http://127.0.0.1:9153/api/v1/cache/flush?name=example.com.

# View cache statistics
curl http://127.0.0.1:9153/api/v1/cache/stats
```

## Monitoring

### Health endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health/live` | Process is alive. Always returns 200 after startup |
| `GET /health/ready` | Zones are loaded and server is accepting queries. Returns 503 during initial load |

### Prometheus metrics

Available at `GET /metrics` on the management HTTP port (default 9153).

Key metrics:

- `dns_queries_total{transport, qtype, response_code}` -- query counter
- `dns_query_duration_seconds{path}` -- latency histogram (authoritative vs recursive)
- `dns_cache_size` -- current cache entries
- `dns_cache_hits_total` / `dns_cache_misses_total` -- cache effectiveness
- `dns_rrl_dropped_total` / `dns_rrl_truncated_total` -- rate limiting activity

### Structured logging

All logs are emitted as JSON by default, suitable for ingestion into ELK, Loki, or any structured log pipeline. Set `level = "debug"` for detailed per-query logging.

## Deployment

### Systemd

Example unit file:

```ini
[Unit]
Description=CERN DNS Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/cern-dns --config /etc/dns/config.toml
Restart=on-failure
RestartSec=5

# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
PrivateTmp=yes
ReadOnlyPaths=/etc/dns
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
```

### Port binding

To bind to privileged ports (53, 443, 853) without running as root, grant the `CAP_NET_BIND_SERVICE` capability:

```bash
sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/cern-dns
```

Or use the `AmbientCapabilities` directive in systemd (shown above).

### Multiple instances

Each instance is fully independent. Zone consistency is achieved by pushing identical zone files to all instances via your configuration management tool. There is no DNS-native replication (no AXFR/IXFR). Deploy behind anycast or a load balancer.

## Troubleshooting

### Server won't start

- **Port in use** -- another process is bound to the configured port. Check with `ss -tlnp | grep :53`.
- **Zone parse error** -- check logs for parse errors. Validate zone files with: `cern-dns --check-zone config/zones/example.com.zone`
- **Config error** -- validate config with: `cern-dns --check-config config/config.toml`

### High latency on recursive queries

- Check cache hit rate via `/metrics` (`dns_cache_hits_total` vs `dns_cache_misses_total`).
- If cache hit rate is low, consider increasing `max_entries` or `min_ttl`.
- Check upstream latency via `dns_recursive_upstream_duration_seconds`.

### Queries refused

- Check ACL configuration in `[policy]`.
- Check logs for `"query refused by ACL"` messages.
- Verify the source IP is in the allowed network range.
