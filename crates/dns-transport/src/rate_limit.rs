use dashmap::DashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tracing::debug;

/// Response Rate Limiting (RRL) engine.
///
/// Implements token-bucket rate limiting per source IP prefix to mitigate
/// DNS amplification attacks and random subdomain attacks.
pub struct RrlEngine {
    /// Rate limit buckets keyed by masked IP prefix.
    buckets: DashMap<IpAddr, RrlBucket>,
    /// Maximum responses per second per prefix.
    responses_per_second: u32,
    /// Slip ratio: 1 in N rate-limited responses gets a truncated response
    /// instead of being dropped (to signal the client to retry via TCP).
    slip: u32,
    /// IPv4 prefix length for grouping (e.g., 24 = /24 network).
    ipv4_prefix_length: u8,
    /// IPv6 prefix length for grouping (e.g., 48 = /48 network).
    ipv6_prefix_length: u8,
    /// Counter for slip decision.
    slip_counter: AtomicU64,
}

struct RrlBucket {
    tokens: f64,
    last_refill: Instant,
}

/// Action to take for a rate-limited response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RrlAction {
    /// Allow the response through.
    Allow,
    /// Drop the response entirely.
    Drop,
    /// Send a truncated response (TC=1) to trigger TCP retry.
    Truncate,
}

impl RrlEngine {
    pub fn new(
        responses_per_second: u32,
        slip: u32,
        ipv4_prefix_length: u8,
        ipv6_prefix_length: u8,
    ) -> Self {
        Self {
            buckets: DashMap::new(),
            responses_per_second,
            slip,
            ipv4_prefix_length,
            ipv6_prefix_length,
            slip_counter: AtomicU64::new(0),
        }
    }

    /// Check whether a response to this source IP should be allowed, dropped, or truncated.
    pub fn check(&self, src: IpAddr) -> RrlAction {
        let prefix = self.mask_ip(src);
        let now = Instant::now();
        let rps = self.responses_per_second as f64;

        let mut entry = self.buckets.entry(prefix).or_insert_with(|| RrlBucket {
            tokens: rps,
            last_refill: now,
        });

        let bucket = entry.value_mut();

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * rps).min(rps);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            RrlAction::Allow
        } else {
            // Rate limited — decide between drop and truncate
            if self.slip > 0 {
                let counter = self.slip_counter.fetch_add(1, Ordering::Relaxed);
                if counter.is_multiple_of(self.slip as u64) {
                    debug!(src = %src, "RRL: truncating response (slip)");
                    return RrlAction::Truncate;
                }
            }
            debug!(src = %src, "RRL: dropping response");
            RrlAction::Drop
        }
    }

    /// Mask an IP address to its configured prefix.
    fn mask_ip(&self, addr: IpAddr) -> IpAddr {
        match addr {
            IpAddr::V4(ip) => {
                let bits = u32::from(ip);
                let mask = if self.ipv4_prefix_length >= 32 {
                    u32::MAX
                } else {
                    u32::MAX << (32 - self.ipv4_prefix_length)
                };
                IpAddr::V4(Ipv4Addr::from(bits & mask))
            }
            IpAddr::V6(ip) => {
                let bits = u128::from(ip);
                let mask = if self.ipv6_prefix_length >= 128 {
                    u128::MAX
                } else {
                    u128::MAX << (128 - self.ipv6_prefix_length)
                };
                IpAddr::V6(Ipv6Addr::from(bits & mask))
            }
        }
    }
}

/// Per-IP connection rate limiter for TCP/DoT.
pub struct ConnectionLimiter {
    connections: DashMap<IpAddr, u32>,
    max_per_ip: u32,
}

impl ConnectionLimiter {
    pub fn new(max_per_ip: u32) -> Self {
        Self {
            connections: DashMap::new(),
            max_per_ip,
        }
    }

    /// Try to acquire a connection slot for the given IP.
    /// Returns true if allowed, false if at the limit.
    pub fn try_acquire(&self, addr: IpAddr) -> bool {
        let mut count = self.connections.entry(addr).or_insert(0);
        if *count.value() >= self.max_per_ip {
            false
        } else {
            *count.value_mut() += 1;
            true
        }
    }

    /// Release a connection slot for the given IP.
    pub fn release(&self, addr: IpAddr) {
        if let Some(mut count) = self.connections.get_mut(&addr) {
            if *count.value() > 0 {
                *count.value_mut() -= 1;
            }
            if *count.value() == 0 {
                drop(count);
                self.connections.remove(&addr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrl_allow_under_limit() {
        let rrl = RrlEngine::new(10, 2, 24, 48);
        let src = "192.168.1.1".parse().unwrap();

        // First 10 queries should be allowed
        for _ in 0..10 {
            assert_eq!(rrl.check(src), RrlAction::Allow);
        }
    }

    #[test]
    fn test_rrl_rate_limit_over_limit() {
        let rrl = RrlEngine::new(5, 2, 24, 48);
        let src = "10.0.0.1".parse().unwrap();

        // Exhaust the bucket
        for _ in 0..5 {
            assert_eq!(rrl.check(src), RrlAction::Allow);
        }

        // Next one should be rate-limited (either drop or truncate)
        let action = rrl.check(src);
        assert!(action == RrlAction::Drop || action == RrlAction::Truncate);
    }

    #[test]
    fn test_rrl_ipv4_prefix_grouping() {
        let rrl = RrlEngine::new(3, 0, 24, 48);
        let src1: IpAddr = "192.168.1.1".parse().unwrap();
        let src2: IpAddr = "192.168.1.200".parse().unwrap();

        // Same /24 — should share the bucket
        for _ in 0..3 {
            rrl.check(src1);
        }
        // src2 is in the same /24, bucket should be exhausted
        assert_eq!(rrl.check(src2), RrlAction::Drop);
    }

    #[test]
    fn test_connection_limiter() {
        let limiter = ConnectionLimiter::new(2);
        let addr: IpAddr = "10.0.0.1".parse().unwrap();

        assert!(limiter.try_acquire(addr));
        assert!(limiter.try_acquire(addr));
        assert!(!limiter.try_acquire(addr)); // at limit

        limiter.release(addr);
        assert!(limiter.try_acquire(addr)); // slot freed
    }
}
