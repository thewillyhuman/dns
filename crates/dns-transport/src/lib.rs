//! Network transport listeners for DNS: UDP, TCP, DoT, DoH.

pub mod doh;
pub mod dot;
pub mod rate_limit;
pub mod tcp;
pub mod udp;

use std::future::Future;
use std::net::SocketAddr;

/// Trait for handling parsed DNS queries from any transport.
pub trait QueryHandler: Send + Sync + 'static {
    /// Handle a raw DNS query and return the response bytes, or None to drop.
    fn handle_query(
        &self,
        raw: &[u8],
        src: SocketAddr,
    ) -> impl Future<Output = Option<Vec<u8>>> + Send;
}
