//! Integration tests for DNSSEC signing and validation.

use dns_dnssec::keys::generate_key;
use dns_dnssec::nsec::generate_nsec_chain;
use dns_dnssec::signer::sign_zone;
use dns_dnssec::verifier::{extract_rrsigs, validate_rrset, ValidationResult};
use hickory_proto::dnssec::Algorithm;
use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::collections::{BTreeMap, HashMap};
use std::net::Ipv4Addr;
use std::time::Duration;

fn make_a_record(name: &str, ip: Ipv4Addr) -> Record {
    Record::from_rdata(Name::from_ascii(name).unwrap(), 300, RData::A(ip.into()))
}

fn build_zone_records() -> HashMap<Name, HashMap<RecordType, Vec<Record>>> {
    let mut records = HashMap::new();

    // A records at various names
    let names_ips = [
        ("example.com.", Ipv4Addr::new(192, 0, 2, 1)),
        ("www.example.com.", Ipv4Addr::new(192, 0, 2, 100)),
        ("mail.example.com.", Ipv4Addr::new(192, 0, 2, 200)),
    ];

    for (name_str, ip) in &names_ips {
        let name = Name::from_ascii(name_str).unwrap();
        records
            .entry(name)
            .or_insert_with(HashMap::new)
            .entry(RecordType::A)
            .or_insert_with(Vec::new)
            .push(make_a_record(name_str, *ip));
    }

    records
}

#[test]
fn test_sign_and_verify_zone_ecdsa() {
    let zone = Name::from_ascii("example.com.").unwrap();
    let zsk = generate_key(
        Algorithm::ECDSAP256SHA256,
        &zone,
        false,
        Duration::from_secs(86400 * 30),
    )
    .unwrap();
    let ksk = generate_key(
        Algorithm::ECDSAP256SHA256,
        &zone,
        true,
        Duration::from_secs(86400 * 365),
    )
    .unwrap();

    let records = build_zone_records();
    let rrsigs = sign_zone(&records, &zone, &zsk, &ksk).unwrap();

    // Should have one RRSIG per RRset
    assert_eq!(rrsigs.len(), 3, "expected 3 RRSIGs (one per A RRset)");

    // Verify each RRset
    for (name, type_map) in &records {
        for (rtype, rrset) in type_map {
            let rrsig_records = extract_rrsigs(&rrsigs);
            let result = validate_rrset(
                name,
                *rtype,
                rrset,
                &rrsig_records,
                std::slice::from_ref(&zsk.dnskey),
            );
            assert_eq!(
                result,
                ValidationResult::Secure,
                "RRset ({}, {:?}) should validate as Secure",
                name,
                rtype
            );
        }
    }
}

#[test]
fn test_sign_and_verify_zone_ed25519() {
    let zone = Name::from_ascii("example.com.").unwrap();
    let zsk = generate_key(Algorithm::ED25519, &zone, false, Duration::from_secs(86400)).unwrap();
    let ksk = generate_key(Algorithm::ED25519, &zone, true, Duration::from_secs(86400)).unwrap();

    let records = build_zone_records();
    let rrsigs = sign_zone(&records, &zone, &zsk, &ksk).unwrap();

    assert!(!rrsigs.is_empty());

    // Verify www.example.com A
    let name = Name::from_ascii("www.example.com.").unwrap();
    let rrset = &records[&name][&RecordType::A];
    let rrsig_records = extract_rrsigs(&rrsigs);
    let result = validate_rrset(
        &name,
        RecordType::A,
        rrset,
        &rrsig_records,
        std::slice::from_ref(&zsk.dnskey),
    );
    assert_eq!(result, ValidationResult::Secure);
}

#[test]
fn test_tampered_record_fails_validation() {
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

    let records = build_zone_records();
    let rrsigs = sign_zone(&records, &zone, &zsk, &ksk).unwrap();

    // Tamper with the record — change the IP
    let name = Name::from_ascii("www.example.com.").unwrap();
    let tampered = vec![make_a_record("www.example.com.", Ipv4Addr::new(1, 2, 3, 4))];

    let rrsig_records = extract_rrsigs(&rrsigs);
    let result = validate_rrset(
        &name,
        RecordType::A,
        &tampered,
        &rrsig_records,
        std::slice::from_ref(&zsk.dnskey),
    );
    assert!(
        matches!(result, ValidationResult::Bogus(_)),
        "tampered record should fail validation, got {:?}",
        result
    );
}

#[test]
fn test_nsec_chain_generation() {
    let origin = Name::from_ascii("example.com.").unwrap();

    let mut records = BTreeMap::new();
    let names = [
        "alpha.example.com.",
        "bravo.example.com.",
        "charlie.example.com.",
    ];
    for name_str in &names {
        let name = Name::from_ascii(name_str).unwrap();
        let record = make_a_record(name_str, Ipv4Addr::new(1, 2, 3, 4));
        records
            .entry(name)
            .or_insert_with(HashMap::new)
            .entry(RecordType::A)
            .or_insert_with(Vec::new)
            .push(record);
    }

    let nsec_chain = generate_nsec_chain(&records, &origin, 300);
    assert_eq!(nsec_chain.len(), 3);

    // All NSEC records should be at the right names
    for (i, name_str) in names.iter().enumerate() {
        assert_eq!(nsec_chain[i].name(), &Name::from_ascii(name_str).unwrap());
    }
}
