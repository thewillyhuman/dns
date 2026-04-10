//! Authoritative DNS engine: zone storage, loading, and query resolution.

pub mod loader;
pub mod lookup;
pub mod negative;
pub mod zone;
pub mod zone_store;

pub use negative::AuthResponse;
pub use zone::Zone;
pub use zone_store::ZoneStore;
