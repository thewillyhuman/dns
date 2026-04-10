//! DNSSEC signing, verification, and key management.

pub mod keys;
pub mod nsec;
pub mod signer;
pub mod verifier;

pub use keys::{generate_key, load_key_from_file, parse_algorithm, DnssecKeyPair};
pub use nsec::{find_covering_nsec, find_nodata_nsec, generate_nsec_chain};
pub use signer::sign_zone;
pub use verifier::{extract_dnskeys, extract_rrsigs, validate_rrset, ValidationResult};
