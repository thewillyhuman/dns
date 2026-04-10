use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{exponential_buckets, Histogram};
use prometheus_client::registry::Registry;
use std::sync::Arc;

/// Labels for query metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct QueryLabels {
    pub transport: String,
    pub qtype: String,
    pub zone: String,
    pub response_code: String,
}

/// Labels for upstream metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct ServerLabels {
    pub server: String,
}

/// Labels for zone metrics.
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct ZoneLabels {
    pub zone: String,
}

/// All DNS server metrics.
pub struct DnsMetrics {
    pub registry: parking_lot::Mutex<Registry>,

    // Query metrics
    pub queries_total: Family<QueryLabels, Counter>,
    pub query_duration_seconds: Histogram,

    // Recursive metrics
    pub recursive_upstream_queries_total: Family<ServerLabels, Counter>,
    pub recursive_upstream_duration_seconds: Histogram,

    // Cache metrics
    pub cache_size: Gauge,
    pub cache_hits_total: Counter,
    pub cache_misses_total: Counter,
    pub cache_evictions_total: Counter,

    // Zone metrics
    pub zone_records_total: Family<ZoneLabels, Gauge>,
    pub zone_reload_errors_total: Family<ZoneLabels, Counter>,

    // System metrics
    pub rrl_dropped_total: Counter,
    pub rrl_truncated_total: Counter,
    pub tcp_connections_active: Gauge,
    pub inflight_recursive_queries: Gauge,
}

impl DnsMetrics {
    pub fn new() -> Arc<Self> {
        let mut registry = Registry::default();

        let queries_total = Family::<QueryLabels, Counter>::default();
        registry.register("dns_queries", "Total DNS queries", queries_total.clone());

        let query_duration_seconds =
            Histogram::new(exponential_buckets(0.0001, 2.0, 16));
        registry.register(
            "dns_query_duration_seconds",
            "Query duration in seconds",
            query_duration_seconds.clone(),
        );

        let recursive_upstream_queries_total =
            Family::<ServerLabels, Counter>::default();
        registry.register(
            "dns_recursive_upstream_queries",
            "Upstream recursive queries",
            recursive_upstream_queries_total.clone(),
        );

        let recursive_upstream_duration_seconds =
            Histogram::new(exponential_buckets(0.001, 2.0, 14));
        registry.register(
            "dns_recursive_upstream_duration_seconds",
            "Upstream query duration",
            recursive_upstream_duration_seconds.clone(),
        );

        let cache_size = Gauge::default();
        registry.register("dns_cache_size", "Current cache entries", cache_size.clone());

        let cache_hits_total = Counter::default();
        registry.register("dns_cache_hits", "Cache hits", cache_hits_total.clone());

        let cache_misses_total = Counter::default();
        registry.register("dns_cache_misses", "Cache misses", cache_misses_total.clone());

        let cache_evictions_total = Counter::default();
        registry.register(
            "dns_cache_evictions",
            "Cache evictions",
            cache_evictions_total.clone(),
        );

        let zone_records_total = Family::<ZoneLabels, Gauge>::default();
        registry.register(
            "dns_zone_records",
            "Records per zone",
            zone_records_total.clone(),
        );

        let zone_reload_errors_total = Family::<ZoneLabels, Counter>::default();
        registry.register(
            "dns_zone_reload_errors",
            "Zone reload errors",
            zone_reload_errors_total.clone(),
        );

        let rrl_dropped_total = Counter::default();
        registry.register("dns_rrl_dropped", "RRL dropped responses", rrl_dropped_total.clone());

        let rrl_truncated_total = Counter::default();
        registry.register(
            "dns_rrl_truncated",
            "RRL truncated responses",
            rrl_truncated_total.clone(),
        );

        let tcp_connections_active = Gauge::default();
        registry.register(
            "dns_tcp_connections_active",
            "Active TCP connections",
            tcp_connections_active.clone(),
        );

        let inflight_recursive_queries = Gauge::default();
        registry.register(
            "dns_inflight_recursive_queries",
            "In-flight recursive queries",
            inflight_recursive_queries.clone(),
        );

        Arc::new(Self {
            registry: parking_lot::Mutex::new(registry),
            queries_total,
            query_duration_seconds,
            recursive_upstream_queries_total,
            recursive_upstream_duration_seconds,
            cache_size,
            cache_hits_total,
            cache_misses_total,
            cache_evictions_total,
            zone_records_total,
            zone_reload_errors_total,
            rrl_dropped_total,
            rrl_truncated_total,
            tcp_connections_active,
            inflight_recursive_queries,
        })
    }

    /// Encode all metrics in Prometheus text format.
    pub fn encode(&self) -> String {
        let mut buf = String::new();
        let registry = self.registry.lock();
        encode(&mut buf, &registry).unwrap();
        buf
    }
}

impl Default for DnsMetrics {
    fn default() -> Self {
        // This won't be used directly — use DnsMetrics::new() which returns Arc
        panic!("Use DnsMetrics::new() instead");
    }
}
