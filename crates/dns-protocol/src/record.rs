use hickory_proto::rr::{Record, RecordType};

/// A set of DNS records of the same type at the same owner name.
pub type RRset = Vec<Record>;

/// Filter records by type.
pub fn rrset_for_type(records: &[Record], rtype: RecordType) -> Vec<&Record> {
    records
        .iter()
        .filter(|r| r.record_type() == rtype)
        .collect()
}

/// Get the minimum TTL from a set of records.
pub fn min_ttl(records: &[Record]) -> u32 {
    records.iter().map(|r| r.ttl()).min().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::{Name, RData};
    use std::net::Ipv4Addr;

    #[test]
    fn test_rrset_for_type() {
        let name = Name::from_ascii("example.com.").unwrap();
        let a_record = Record::from_rdata(
            name.clone(),
            300,
            RData::A(Ipv4Addr::new(1, 2, 3, 4).into()),
        );
        let aaaa_record = Record::update0(name.clone(), 300, RecordType::AAAA);

        let records = vec![a_record, aaaa_record];
        let a_set = rrset_for_type(&records, RecordType::A);
        assert_eq!(a_set.len(), 1);
    }
}
