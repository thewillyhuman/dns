//! DNSSEC signing, verification, and key management.

pub mod keys;
pub mod nsec;
pub mod signer;
pub mod verifier;

pub use keys::{DnssecKeyPair, generate_key, load_key_from_file, parse_algorithm};
pub use signer::sign_zone;
pub use verifier::{ValidationResult, validate_rrset, extract_rrsigs, extract_dnskeys};
pub use nsec::{generate_nsec_chain, find_covering_nsec, find_nodata_nsec};
