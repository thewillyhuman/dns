use crate::keys::DnssecKeyPair;
use hickory_proto::dnssec::rdata::{DNSSECRData, RRSIG, SIG};
use hickory_proto::dnssec::tbs::TBS;
use hickory_proto::dnssec::Algorithm;
use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum SignError {
    #[error("signing failed: {0}")]
    Signing(String),
    #[error("no ZSK available for signing")]
    NoZsk,
    #[error("no KSK available for signing DNSKEY RRset")]
    NoKsk,
}

/// Sign all RRsets in a zone, producing RRSIG records.
///
/// Returns a list of RRSIG records to add to the zone.
pub fn sign_zone(
    records: &HashMap<Name, HashMap<RecordType, Vec<Record>>>,
    origin: &Name,
    zsk: &DnssecKeyPair,
    ksk: &DnssecKeyPair,
) -> Result<Vec<Record>, SignError> {
    let mut rrsigs = Vec::new();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    let inception = now;
    let expiration = inception + zsk.signer.sig_duration().as_secs() as u32;

    for (name, type_map) in records {
        for (rtype, rrset) in type_map {
            if rrset.is_empty() {
                continue;
            }
            // Skip RRSIG records themselves
            if *rtype == RecordType::RRSIG {
                continue;
            }

            // DNSKEY RRset at apex is signed with KSK
            let signer = if *rtype == RecordType::DNSKEY && name == origin {
                &ksk.signer
            } else {
                &zsk.signer
            };

            let key_tag = signer
                .calculate_key_tag()
                .map_err(|e| SignError::Signing(e.to_string()))?;

            match sign_rrset(
                rrset,
                name,
                *rtype,
                signer.key().algorithm(),
                key_tag,
                signer.signer_name(),
                inception,
                expiration,
                signer,
            ) {
                Ok(rrsig_record) => {
                    rrsigs.push(rrsig_record);
                }
                Err(e) => {
                    warn!(name = %name, rtype = ?rtype, error = %e, "failed to sign RRset");
                }
            }
        }
    }

    debug!(
        zone = %origin,
        rrsig_count = rrsigs.len(),
        "zone signing complete"
    );

    Ok(rrsigs)
}

/// Sign a single RRset and produce an RRSIG record.
fn sign_rrset(
    records: &[Record],
    name: &Name,
    rtype: RecordType,
    algorithm: Algorithm,
    key_tag: u16,
    signer_name: &Name,
    inception: u32,
    expiration: u32,
    signer: &hickory_proto::dnssec::SigSigner,
) -> Result<Record, SignError> {
    let ttl = records.first().map(|r| r.ttl()).unwrap_or(300);
    let num_labels = name.num_labels();

    // Build a pre-RRSIG (SIG structure) for TBS computation
    let pre_sig = SIG::new(
        rtype,
        algorithm,
        num_labels,
        ttl,
        expiration,
        inception,
        key_tag,
        signer_name.clone(),
        Vec::new(), // signature placeholder
    );

    // Build TBS data from the SIG + records
    let tbs = TBS::from_sig(name, DNSClass::IN, &pre_sig, records.iter())
        .map_err(|e| SignError::Signing(e.to_string()))?;

    // Sign
    let signature = signer
        .sign(&tbs)
        .map_err(|e| SignError::Signing(e.to_string()))?;

    // Build RRSIG record
    let rrsig = RRSIG::new(
        rtype,
        algorithm,
        num_labels,
        ttl,
        expiration,
        inception,
        key_tag,
        signer_name.clone(),
        signature,
    );

    Ok(Record::from_rdata(
        name.clone(),
        ttl,
        RData::DNSSEC(DNSSECRData::RRSIG(rrsig)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::generate_key;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    fn make_a_record(name: &str, ip: Ipv4Addr) -> Record {
        Record::from_rdata(
            Name::from_ascii(name).unwrap(),
            300,
            RData::A(ip.into()),
        )
    }

    #[test]
    fn test_sign_zone() {
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

        let mut records: HashMap<Name, HashMap<RecordType, Vec<Record>>> = HashMap::new();
        let name = Name::from_ascii("www.example.com.").unwrap();
        records
            .entry(name)
            .or_default()
            .entry(RecordType::A)
            .or_default()
            .push(make_a_record("www.example.com.", Ipv4Addr::new(1, 2, 3, 4)));

        let rrsigs = sign_zone(&records, &zone, &zsk, &ksk).unwrap();
        assert_eq!(rrsigs.len(), 1);

        // Verify the RRSIG record has correct type
        match rrsigs[0].data() {
            RData::DNSSEC(DNSSECRData::RRSIG(rrsig)) => {
                assert_eq!(rrsig.type_covered(), RecordType::A);
                assert_eq!(rrsig.algorithm(), Algorithm::ECDSAP256SHA256);
            }
            _ => panic!("expected RRSIG record"),
        }
    }
}
