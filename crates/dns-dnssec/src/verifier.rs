use hickory_proto::dnssec::rdata::{DNSSECRData, DNSKEY, RRSIG};
use hickory_proto::dnssec::Verifier as HickoryVerifier;
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Result of DNSSEC validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// The RRset was validated successfully.
    Secure,
    /// No DNSSEC records present — cannot validate.
    Insecure,
    /// Validation failed — the data should not be trusted.
    Bogus(String),
}

/// Validate an RRset against RRSIG and DNSKEY records.
pub fn validate_rrset(
    name: &Name,
    rtype: RecordType,
    rrset: &[Record],
    rrsigs: &[RRSIG],
    dnskeys: &[DNSKEY],
) -> ValidationResult {
    if rrsigs.is_empty() {
        return ValidationResult::Insecure;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;

    for rrsig in rrsigs {
        // Check that this RRSIG covers the right type
        if rrsig.type_covered() != rtype {
            continue;
        }

        // Check time validity
        if now > rrsig.sig_expiration().get() {
            debug!(name = %name, rtype = ?rtype, "RRSIG expired");
            continue;
        }
        if now < rrsig.sig_inception().get() {
            debug!(name = %name, rtype = ?rtype, "RRSIG not yet valid");
            continue;
        }

        // Find matching DNSKEY
        let key_tag = rrsig.key_tag();
        for dnskey in dnskeys {
            if !dnskey.zone_key() || dnskey.algorithm() != rrsig.algorithm() {
                continue;
            }

            match dnskey.verify_rrsig(name, DNSClass::IN, rrsig, rrset.iter()) {
                Ok(()) => {
                    debug!(
                        name = %name,
                        rtype = ?rtype,
                        key_tag = key_tag,
                        "DNSSEC validation succeeded"
                    );
                    return ValidationResult::Secure;
                }
                Err(e) => {
                    debug!(
                        name = %name,
                        rtype = ?rtype,
                        key_tag = key_tag,
                        error = %e,
                        "RRSIG verification failed, trying next key"
                    );
                }
            }
        }
    }

    let reason = format!("no valid RRSIG found for ({}, {:?})", name, rtype);
    warn!(name = %name, rtype = ?rtype, "DNSSEC validation failed");
    ValidationResult::Bogus(reason)
}

/// Extract RRSIG records from a list of records.
pub fn extract_rrsigs(records: &[Record]) -> Vec<RRSIG> {
    records
        .iter()
        .filter_map(|r| match r.data() {
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) => Some(rrsig.clone()),
            _ => None,
        })
        .collect()
}

/// Extract DNSKEY records from a list of records.
pub fn extract_dnskeys(records: &[Record]) -> Vec<DNSKEY> {
    records
        .iter()
        .filter_map(|r| match r.data() {
            RData::DNSSEC(DNSSECRData::DNSKEY(dnskey)) => Some(dnskey.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::generate_key;
    use crate::signer::sign_zone;
    use hickory_proto::dnssec::Algorithm;
    use std::collections::HashMap;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    fn make_a_record(name: &str, ip: Ipv4Addr) -> Record {
        Record::from_rdata(Name::from_ascii(name).unwrap(), 300, RData::A(ip.into()))
    }

    #[test]
    fn test_sign_and_verify() {
        let zone = Name::from_ascii("example.com.").unwrap();
        let zsk = generate_key(
            Algorithm::ECDSAP256SHA256,
            &zone,
            false,
            Duration::from_secs(86400),
        )
        .unwrap();
        let ksk = generate_key(
            Algorithm::ECDSAP256SHA256,
            &zone,
            true,
            Duration::from_secs(86400),
        )
        .unwrap();

        let name = Name::from_ascii("www.example.com.").unwrap();
        let a_record = make_a_record("www.example.com.", Ipv4Addr::new(1, 2, 3, 4));

        let mut records: HashMap<Name, HashMap<RecordType, Vec<Record>>> = HashMap::new();
        records
            .entry(name.clone())
            .or_default()
            .entry(RecordType::A)
            .or_default()
            .push(a_record.clone());

        let rrsigs = sign_zone(&records, &zone, &zsk, &ksk).unwrap();
        let rrsig_records = extract_rrsigs(&rrsigs);

        let result = validate_rrset(
            &name,
            RecordType::A,
            &[a_record],
            &rrsig_records,
            std::slice::from_ref(&zsk.dnskey),
        );
        assert_eq!(result, ValidationResult::Secure);
    }

    #[test]
    fn test_no_rrsig_is_insecure() {
        let name = Name::from_ascii("example.com.").unwrap();
        let record = make_a_record("example.com.", Ipv4Addr::new(1, 2, 3, 4));
        let result = validate_rrset(&name, RecordType::A, &[record], &[], &[]);
        assert_eq!(result, ValidationResult::Insecure);
    }
}
