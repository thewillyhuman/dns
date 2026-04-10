use hickory_proto::dnssec::rdata::{DNSSECRData, NSEC};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::collections::HashMap;
use tracing::debug;

/// Generate NSEC records for the entire zone (RFC 4034 §4).
///
/// NSEC records form a chain covering all names in the zone in canonical order.
/// Each NSEC record points to the next name and lists the types present at
/// the current name.
pub fn generate_nsec_chain(
    records: &HashMap<Name, HashMap<RecordType, Vec<Record>>>,
    origin: &Name,
    ttl: u32,
) -> Vec<Record> {
    if records.is_empty() {
        return Vec::new();
    }

    // Sort names into canonical order for the NSEC chain
    let mut names: Vec<&Name> = records.keys().collect();
    names.sort();
    let mut nsec_records = Vec::new();

    for (i, name) in names.iter().enumerate() {
        let next_name = if i + 1 < names.len() {
            names[i + 1].clone()
        } else {
            // Last name wraps back to zone origin
            origin.clone()
        };

        // Collect all record types at this name
        let mut types: Vec<RecordType> = records[*name].keys().copied().collect();
        // NSEC itself is present at every name in the chain
        types.push(RecordType::NSEC);
        // RRSIG is present at every name (once signed)
        types.push(RecordType::RRSIG);
        types.sort();
        types.dedup();

        let nsec = NSEC::new(next_name, types);
        let record =
            Record::from_rdata((*name).clone(), ttl, RData::DNSSEC(DNSSECRData::NSEC(nsec)));
        nsec_records.push(record);
    }

    debug!(
        zone = %origin,
        nsec_count = nsec_records.len(),
        "NSEC chain generated"
    );

    nsec_records
}

/// Find the NSEC record that proves a name does not exist (NXDOMAIN proof).
///
/// Returns the NSEC record whose owner name is the greatest name
/// that is less than the queried name.
pub fn find_covering_nsec<'a>(nsec_records: &'a [Record], qname: &Name) -> Option<&'a Record> {
    // Find NSEC where owner < qname < next_domain_name
    nsec_records.iter().find(|record| {
        let owner = record.name();
        if let RData::DNSSEC(DNSSECRData::NSEC(nsec)) = record.data() {
            let next = nsec.next_domain_name();
            // Normal case: owner < qname < next
            if owner < qname && (qname < next || next <= owner) {
                return true;
            }
            // Wrap-around case: next <= owner (last in chain)
            if next <= owner && (qname > owner || qname < next) {
                return true;
            }
        }
        false
    })
}

/// Find the NSEC record that proves a type does not exist at a name (NODATA proof).
///
/// Returns the NSEC record for the exact name if it exists but doesn't
/// contain the queried type.
pub fn find_nodata_nsec<'a>(
    nsec_records: &'a [Record],
    qname: &Name,
    qtype: RecordType,
) -> Option<&'a Record> {
    nsec_records.iter().find(|record| {
        if record.name() != qname {
            return false;
        }
        if let RData::DNSSEC(DNSSECRData::NSEC(nsec)) = record.data() {
            return !nsec.type_bit_maps().any(|t| t == qtype);
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn make_records() -> HashMap<Name, HashMap<RecordType, Vec<Record>>> {
        let mut records = HashMap::new();

        let names = ["a.example.com.", "b.example.com.", "d.example.com."];
        for name_str in &names {
            let name = Name::from_ascii(name_str).unwrap();
            let record = Record::from_rdata(
                name.clone(),
                300,
                RData::A(Ipv4Addr::new(1, 2, 3, 4).into()),
            );
            records
                .entry(name)
                .or_insert_with(HashMap::new)
                .entry(RecordType::A)
                .or_insert_with(Vec::new)
                .push(record);
        }

        records
    }

    #[test]
    fn test_generate_nsec_chain() {
        let origin = Name::from_ascii("example.com.").unwrap();
        let records = make_records();
        let nsec_records = generate_nsec_chain(&records, &origin, 300);

        assert_eq!(nsec_records.len(), 3);

        // First NSEC: a.example.com -> b.example.com
        assert_eq!(
            nsec_records[0].name(),
            &Name::from_ascii("a.example.com.").unwrap()
        );
        if let RData::DNSSEC(DNSSECRData::NSEC(nsec)) = nsec_records[0].data() {
            assert_eq!(
                nsec.next_domain_name(),
                &Name::from_ascii("b.example.com.").unwrap()
            );
        }

        // Last NSEC wraps to origin
        if let RData::DNSSEC(DNSSECRData::NSEC(nsec)) = nsec_records[2].data() {
            assert_eq!(nsec.next_domain_name(), &origin);
        }
    }

    #[test]
    fn test_find_covering_nsec() {
        let origin = Name::from_ascii("example.com.").unwrap();
        let records = make_records();
        let nsec_records = generate_nsec_chain(&records, &origin, 300);

        // c.example.com doesn't exist, should be covered by b -> d
        let qname = Name::from_ascii("c.example.com.").unwrap();
        let covering = find_covering_nsec(&nsec_records, &qname);
        assert!(covering.is_some());
        assert_eq!(
            covering.unwrap().name(),
            &Name::from_ascii("b.example.com.").unwrap()
        );
    }

    #[test]
    fn test_find_nodata_nsec() {
        let origin = Name::from_ascii("example.com.").unwrap();
        let records = make_records();
        let nsec_records = generate_nsec_chain(&records, &origin, 300);

        // a.example.com exists but has no AAAA record
        let qname = Name::from_ascii("a.example.com.").unwrap();
        let nodata = find_nodata_nsec(&nsec_records, &qname, RecordType::AAAA);
        assert!(nodata.is_some());

        // a.example.com has A record
        let has_a = find_nodata_nsec(&nsec_records, &qname, RecordType::A);
        assert!(has_a.is_none()); // A IS in the type bitmap
    }
}
