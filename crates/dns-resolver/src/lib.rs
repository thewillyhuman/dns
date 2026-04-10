//! Recursive DNS resolver with caching and query deduplication.

pub mod cache;
pub mod dedup;
pub mod qname_min;
pub mod resolver;
pub mod upstream;
pub mod validator;

pub use cache::DnsCache;
pub use resolver::{ResolveError, ResolveResponse, Resolver};
