use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::collections::{BTreeMap, HashMap};

/// A DNS zone containing all records organized by owner name and record type.
#[derive(Debug, Clone)]
pub struct Zone {
    /// The zone origin (apex name).
    pub origin: Name,
    /// SOA record for this zone.
    pub soa: Record,
    /// All records indexed by owner name, then by record type.
    pub records: BTreeMap<Name, HashMap<RecordType, Vec<Record>>>,
}

impl Zone {
    pub fn new(origin: Name, soa: Record) -> Self {
        let mut records = BTreeMap::new();
        records
            .entry(origin.clone())
            .or_insert_with(HashMap::new)
            .entry(RecordType::SOA)
            .or_insert_with(Vec::new)
            .push(soa.clone());
        Self {
            origin,
            soa,
            records,
        }
    }

    /// Add a record to the zone.
    pub fn add_record(&mut self, record: Record) {
        self.records
            .entry(record.name().clone())
            .or_default()
            .entry(record.record_type())
            .or_default()
            .push(record);
    }

    /// Look up records at an exact name and type.
    pub fn lookup_exact(&self, name: &Name, rtype: RecordType) -> Option<&Vec<Record>> {
        self.records.get(name).and_then(|types| types.get(&rtype))
    }

    /// Check if a name exists in this zone (has any records).
    pub fn name_exists(&self, name: &Name) -> bool {
        self.records.contains_key(name)
    }

    /// Get NS records at the zone apex.
    pub fn apex_ns(&self) -> Option<&Vec<Record>> {
        self.lookup_exact(&self.origin, RecordType::NS)
    }

    /// Find a delegation point for a query name.
    /// Returns the NS records at the closest enclosing delegation if one exists.
    pub fn find_delegation(&self, qname: &Name) -> Option<(&Name, &Vec<Record>)> {
        // Walk from the query name up toward the origin, looking for NS records
        // at intermediate names (but NOT at the origin itself, since that's the zone apex).
        let mut current = qname.clone();
        while current != self.origin && dns_protocol::name::is_subdomain(&current, &self.origin) {
            if let Some(parent) = dns_protocol::name::parent(&current) {
                if parent == self.origin {
                    break;
                }
                if let Some(ns_records) = self.lookup_exact(&parent, RecordType::NS) {
                    if let Some((name, _)) = self.records.get_key_value(&parent) {
                        return Some((name, ns_records));
                    }
                }
                current = parent;
            } else {
                break;
            }
        }
        // Also check the immediate name if it's not the origin
        if qname != &self.origin {
            // Check parents between qname and origin for NS
            let mut check = qname.clone();
            loop {
                if check == self.origin {
                    break;
                }
                if let Some(ns) = self.lookup_exact(&check, RecordType::NS) {
                    if &check != qname {
                        // Delegation at an intermediate name
                        if let Some((name, _)) = self.records.get_key_value(&check) {
                            return Some((name, ns));
                        }
                    }
                }
                match dns_protocol::name::parent(&check) {
                    Some(p) => check = p,
                    None => break,
                }
            }
        }
        None
    }

    /// Find the wildcard name that could match qname, if any.
    /// Per RFC 4592: find the closest encloser, then try *.closest_encloser
    pub fn find_wildcard(&self, qname: &Name) -> Option<&HashMap<RecordType, Vec<Record>>> {
        let mut current = qname.clone();
        loop {
            match dns_protocol::name::parent(&current) {
                Some(parent) => {
                    if !dns_protocol::name::is_subdomain(&parent, &self.origin)
                        && parent != self.origin
                    {
                        return None;
                    }
                    // Try *.parent
                    let wildcard = Name::from_ascii(format!("*.{}", parent)).ok()?;
                    if let Some(types) = self.records.get(&wildcard) {
                        return Some(types);
                    }
                    if parent == self.origin {
                        return None;
                    }
                    current = parent;
                }
                None => return None,
            }
        }
    }

    /// Get glue records (A/AAAA) for a given name if they exist in this zone.
    pub fn glue_records(&self, name: &Name) -> Vec<Record> {
        let mut glue = Vec::new();
        if let Some(a_records) = self.lookup_exact(name, RecordType::A) {
            glue.extend(a_records.iter().cloned());
        }
        if let Some(aaaa_records) = self.lookup_exact(name, RecordType::AAAA) {
            glue.extend(aaaa_records.iter().cloned());
        }
        glue
    }

    /// Total number of records in the zone.
    pub fn record_count(&self) -> usize {
        self.records
            .values()
            .flat_map(|types| types.values())
            .map(|rrset| rrset.len())
            .sum()
    }

    /// Get the SOA serial number.
    pub fn serial(&self) -> u32 {
        if let RData::SOA(soa) = self.soa.data() {
            soa.serial()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::rdata::SOA;
    use std::net::Ipv4Addr;

    fn test_zone() -> Zone {
        let origin = Name::from_ascii("example.com.").unwrap();
        let soa_rdata = SOA::new(
            Name::from_ascii("ns1.example.com.").unwrap(),
            Name::from_ascii("admin.example.com.").unwrap(),
            2024010101,
            3600,
            900,
            604800,
            86400,
        );
        let soa = Record::from_rdata(origin.clone(), 3600, RData::SOA(soa_rdata));
        let mut zone = Zone::new(origin.clone(), soa);

        // Add NS records
        zone.add_record(Record::from_rdata(
            origin.clone(),
            3600,
            RData::NS(hickory_proto::rr::rdata::NS(
                Name::from_ascii("ns1.example.com.").unwrap(),
            )),
        ));

        // Add A record at apex
        zone.add_record(Record::from_rdata(
            origin.clone(),
            300,
            RData::A(Ipv4Addr::new(1, 2, 3, 4).into()),
        ));

        // Add www A record
        let www = Name::from_ascii("www.example.com.").unwrap();
        zone.add_record(Record::from_rdata(
            www.clone(),
            300,
            RData::A(Ipv4Addr::new(1, 2, 3, 5).into()),
        ));

        // Add wildcard
        let wild = Name::from_ascii("*.wild.example.com.").unwrap();
        zone.add_record(Record::from_rdata(
            wild,
            300,
            RData::A(Ipv4Addr::new(1, 2, 3, 200).into()),
        ));

        // Add CNAME
        let alias = Name::from_ascii("alias.example.com.").unwrap();
        zone.add_record(Record::from_rdata(
            alias,
            300,
            RData::CNAME(hickory_proto::rr::rdata::CNAME(www)),
        ));

        zone
    }

    #[test]
    fn test_exact_lookup() {
        let zone = test_zone();
        let www = Name::from_ascii("www.example.com.").unwrap();
        let result = zone.lookup_exact(&www, RecordType::A);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_name_exists() {
        let zone = test_zone();
        assert!(zone.name_exists(&Name::from_ascii("www.example.com.").unwrap()));
        assert!(!zone.name_exists(&Name::from_ascii("nonexistent.example.com.").unwrap()));
    }

    #[test]
    fn test_wildcard() {
        let zone = test_zone();
        let qname = Name::from_ascii("foo.wild.example.com.").unwrap();
        let result = zone.find_wildcard(&qname);
        assert!(result.is_some());
        let types = result.unwrap();
        assert!(types.contains_key(&RecordType::A));
    }

    #[test]
    fn test_no_wildcard_exact() {
        let zone = test_zone();
        // www.example.com exists exactly, wildcard shouldn't be used
        // but find_wildcard doesn't check for exact matches — that's the caller's job
        let qname = Name::from_ascii("nonexistent.example.com.").unwrap();
        let result = zone.find_wildcard(&qname);
        // No wildcard at *.example.com., so should be None
        assert!(result.is_none());
    }

    #[test]
    fn test_record_count() {
        let zone = test_zone();
        assert!(zone.record_count() > 0);
    }

    #[test]
    fn test_serial() {
        let zone = test_zone();
        assert_eq!(zone.serial(), 2024010101);
    }
}
