use hickory_proto::dnssec::crypto::signing_key_from_der;
use hickory_proto::dnssec::rdata::DNSKEY;
use hickory_proto::dnssec::{Algorithm, SigSigner};
use hickory_proto::rr::Name;
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, Ed25519KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};
use rustls_pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("unsupported algorithm: {0}")]
    UnsupportedAlgorithm(String),
    #[error("key generation failed: {0}")]
    Generation(String),
    #[error("key loading failed: {0}")]
    Loading(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A DNSSEC key pair with metadata.
pub struct DnssecKeyPair {
    pub algorithm: Algorithm,
    pub is_ksk: bool,
    pub key_tag: u16,
    pub signer: SigSigner,
    pub dnskey: DNSKEY,
}

/// Parse an algorithm string from config into a hickory Algorithm.
pub fn parse_algorithm(s: &str) -> Result<Algorithm, KeyError> {
    match s.to_uppercase().as_str() {
        "ECDSAP256SHA256" | "13" => Ok(Algorithm::ECDSAP256SHA256),
        "ED25519" | "15" => Ok(Algorithm::ED25519),
        "RSASHA256" | "8" => Ok(Algorithm::RSASHA256),
        _ => Err(KeyError::UnsupportedAlgorithm(s.to_string())),
    }
}

/// Generate a new DNSSEC key pair in memory.
pub fn generate_key(
    algorithm: Algorithm,
    zone: &Name,
    is_ksk: bool,
    sig_duration: Duration,
) -> Result<DnssecKeyPair, KeyError> {
    let rng = SystemRandom::new();

    let pkcs8_bytes = match algorithm {
        Algorithm::ECDSAP256SHA256 => {
            EcdsaKeyPair::generate_pkcs8(
                &ECDSA_P256_SHA256_FIXED_SIGNING,
                &rng,
            )
            .map_err(|e| KeyError::Generation(e.to_string()))?
            .as_ref()
            .to_vec()
        }
        Algorithm::ED25519 => {
            Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|e| KeyError::Generation(e.to_string()))?
                .as_ref()
                .to_vec()
        }
        _ => return Err(KeyError::UnsupportedAlgorithm(format!("{:?}", algorithm))),
    };

    build_key_pair(&pkcs8_bytes, algorithm, zone, is_ksk, sig_duration)
}

/// Load a DNSSEC key pair from a PKCS#8 DER file.
pub fn load_key_from_file(
    path: &Path,
    algorithm: Algorithm,
    zone: &Name,
    is_ksk: bool,
    sig_duration: Duration,
) -> Result<DnssecKeyPair, KeyError> {
    let der_bytes = std::fs::read(path)?;
    build_key_pair(&der_bytes, algorithm, zone, is_ksk, sig_duration)
}

fn build_key_pair(
    pkcs8_bytes: &[u8],
    algorithm: Algorithm,
    zone: &Name,
    is_ksk: bool,
    sig_duration: Duration,
) -> Result<DnssecKeyPair, KeyError> {
    let der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pkcs8_bytes.to_vec()));
    let signing_key = signing_key_from_der(&der, algorithm)
        .map_err(|e| KeyError::Generation(e.to_string()))?;

    let public_key_buf = signing_key
        .to_public_key()
        .map_err(|e| KeyError::Generation(e.to_string()))?;

    let dnskey_rdata = DNSKEY::new(
        /* zone_key */ true,
        /* secure_entry_point (KSK) */ is_ksk,
        /* revoke */ false,
        public_key_buf,
    );

    let signer = SigSigner::dnssec(
        dnskey_rdata.clone(),
        signing_key,
        zone.clone(),
        sig_duration,
    );

    let key_tag = signer
        .calculate_key_tag()
        .map_err(|e| KeyError::Generation(e.to_string()))?;

    let dnskey = signer
        .to_dnskey()
        .map_err(|e| KeyError::Generation(e.to_string()))?;

    info!(
        algorithm = ?algorithm,
        is_ksk = is_ksk,
        key_tag = key_tag,
        zone = %zone,
        "generated DNSSEC key"
    );

    Ok(DnssecKeyPair {
        algorithm,
        is_ksk,
        key_tag,
        signer,
        dnskey,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_ecdsa_zsk() {
        let zone = Name::from_ascii("example.com.").unwrap();
        let key = generate_key(
            Algorithm::ECDSAP256SHA256,
            &zone,
            false,
            Duration::from_secs(86400),
        )
        .unwrap();
        assert!(!key.is_ksk);
        assert_eq!(key.algorithm, Algorithm::ECDSAP256SHA256);
        assert!(key.key_tag > 0);
    }

    #[test]
    fn test_generate_ecdsa_ksk() {
        let zone = Name::from_ascii("example.com.").unwrap();
        let key = generate_key(
            Algorithm::ECDSAP256SHA256,
            &zone,
            true,
            Duration::from_secs(86400),
        )
        .unwrap();
        assert!(key.is_ksk);
        assert!(key.dnskey.secure_entry_point());
    }

    #[test]
    fn test_generate_ed25519() {
        let zone = Name::from_ascii("example.com.").unwrap();
        let key = generate_key(
            Algorithm::ED25519,
            &zone,
            false,
            Duration::from_secs(86400),
        )
        .unwrap();
        assert_eq!(key.algorithm, Algorithm::ED25519);
    }

    #[test]
    fn test_parse_algorithm() {
        assert_eq!(parse_algorithm("ECDSAP256SHA256").unwrap(), Algorithm::ECDSAP256SHA256);
        assert_eq!(parse_algorithm("ED25519").unwrap(), Algorithm::ED25519);
        assert_eq!(parse_algorithm("13").unwrap(), Algorithm::ECDSAP256SHA256);
        assert!(parse_algorithm("UNKNOWN").is_err());
    }
}
