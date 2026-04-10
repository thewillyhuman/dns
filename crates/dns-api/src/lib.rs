//! HTTP management API, health endpoints, and Prometheus metrics.

pub mod health;
pub mod metrics;
pub mod routes;

pub use metrics::DnsMetrics;
pub use routes::{build_router, AppState};
