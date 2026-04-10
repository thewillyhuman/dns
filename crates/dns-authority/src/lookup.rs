use crate::negative::{self, AuthResponse};
use crate::zone::Zone;
use hickory_proto::rr::{Name, RData, Record, RecordType};

const MAX_CNAME_CHAIN: usize = 8;

/// Resolve a query against an authoritative zone.
/// Implements the algorithm from RFC 1034 section 4.3.2.
pub fn resolve_query(zone: &Zone, qname: &Name, qtype: RecordType) -> AuthResponse {
    // 1. Check for delegation first (NS at an intermediate name)
    if qname != &zone.origin {
        if let Some(delegation) = find_delegation_for(zone, qname) {
            return delegation;
        }
    }

    // 2. Exact match
    if zone.name_exists(qname) {
        return resolve_exact(zone, qname, qtype);
    }

    // 3. Wildcard expansion
    if let Some(wildcard_types) = zone.find_wildcard(qname) {
        return resolve_wildcard(zone, qname, qtype, wildcard_types);
    }

    // 4. NXDOMAIN
    negative::nxdomain_response(zone)
}

fn resolve_exact(zone: &Zone, qname: &Name, qtype: RecordType) -> AuthResponse {
    // Check for exact type match
    if let Some(records) = zone.lookup_exact(qname, qtype) {
        let mut additional = Vec::new();
        collect_additional(zone, records, &mut additional);
        return AuthResponse::noerror(records.clone(), authority_ns(zone), additional);
    }

    // Check for CNAME (and qtype is not CNAME itself)
    if qtype != RecordType::CNAME {
        if let Some(cname_records) = zone.lookup_exact(qname, RecordType::CNAME) {
            return chase_cname(zone, qtype, cname_records);
        }
    }

    // Name exists but type doesn't -> NODATA
    negative::nodata_response(zone, qname)
}

fn chase_cname(zone: &Zone, qtype: RecordType, cname_records: &[Record]) -> AuthResponse {
    let mut answers = Vec::new();
    let mut current_cnames = cname_records.to_vec();

    for _ in 0..MAX_CNAME_CHAIN {
        answers.extend(current_cnames.iter().cloned());

        // Get the CNAME target
        let target = match current_cnames.first().map(|r| r.data()) {
            Some(RData::CNAME(cname)) => cname.0.clone(),
            _ => break,
        };

        // Try to resolve the target within this zone
        if let Some(target_records) = zone.lookup_exact(&target, qtype) {
            answers.extend(target_records.iter().cloned());
            return AuthResponse::noerror(answers, authority_ns(zone), Vec::new());
        }

        // Check if target has another CNAME
        match zone.lookup_exact(&target, RecordType::CNAME) {
            Some(next_cnames) => current_cnames = next_cnames.clone(),
            None => break,
        }
    }

    // CNAME exists but target can't be resolved in-zone
    AuthResponse::noerror(answers, authority_ns(zone), Vec::new())
}

fn resolve_wildcard(
    zone: &Zone,
    qname: &Name,
    qtype: RecordType,
    wildcard_types: &std::collections::HashMap<RecordType, Vec<Record>>,
) -> AuthResponse {
    if let Some(records) = wildcard_types.get(&qtype) {
        let synthesized: Vec<Record> = records
            .iter()
            .map(|r| {
                let mut synth = r.clone();
                synth.set_name(qname.clone());
                synth
            })
            .collect();
        return AuthResponse::noerror(synthesized, authority_ns(zone), Vec::new());
    }

    // Check for CNAME in wildcard
    if qtype != RecordType::CNAME {
        if let Some(cname_records) = wildcard_types.get(&RecordType::CNAME) {
            let synthesized: Vec<Record> = cname_records
                .iter()
                .map(|r| {
                    let mut synth = r.clone();
                    synth.set_name(qname.clone());
                    synth
                })
                .collect();
            return AuthResponse::noerror(synthesized, authority_ns(zone), Vec::new());
        }
    }

    // Wildcard exists but no matching type -> NODATA
    negative::nodata_response(zone, qname)
}

fn find_delegation_for(zone: &Zone, qname: &Name) -> Option<AuthResponse> {
    // Walk from just above the origin up to qname looking for NS at intermediate names
    let mut current = qname.clone();
    loop {
        if current == zone.origin {
            break;
        }
        if !dns_protocol::name::is_subdomain(&current, &zone.origin) {
            break;
        }
        // Check if current name has NS records (delegation point)
        // Only if current != qname (we don't delegate at the query name itself for NS queries)
        if let Some(ns_records) = zone.lookup_exact(&current, RecordType::NS) {
            if &current != qname {
                let mut glue = Vec::new();
                for ns in ns_records {
                    if let RData::NS(ns_name) = ns.data() {
                        glue.extend(zone.glue_records(&ns_name.0));
                    }
                }
                return Some(negative::referral_response(ns_records, glue));
            }
        }
        match dns_protocol::name::parent(&current) {
            Some(parent) => current = parent,
            None => break,
        }
    }
    None
}

fn authority_ns(zone: &Zone) -> Vec<Record> {
    zone.apex_ns().cloned().unwrap_or_default()
}

fn collect_additional(zone: &Zone, records: &[Record], additional: &mut Vec<Record>) {
    for record in records {
        let target = match record.data() {
            RData::MX(mx) => Some(mx.exchange().clone()),
            RData::NS(ns) => Some(ns.0.clone()),
            RData::SRV(srv) => Some(srv.target().clone()),
            _ => None,
        };
        if let Some(target) = target {
            additional.extend(zone.glue_records(&target));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::parse_zone_str;
    use hickory_proto::op::ResponseCode;
    use std::path::PathBuf;

    const TEST_ZONE: &str = r#"
$ORIGIN example.com.
$TTL 3600

@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101 3600 900 604800 86400 )

    IN  NS  ns1.example.com.
    IN  NS  ns2.example.com.
    IN  A   192.0.2.1

ns1 IN  A   192.0.2.10
ns2 IN  A   192.0.2.11

www IN  A   192.0.2.100
    IN  A   192.0.2.101

mail    IN  A       192.0.2.50
@       IN  MX  10  mail.example.com.

ftp IN  CNAME   www.example.com.

alias IN CNAME ftp.example.com.

*.wild  IN  A   192.0.2.200

info    IN  TXT "Example zone for testing"
"#;

    fn load_test_zone() -> Zone {
        parse_zone_str(TEST_ZONE, &PathBuf::from("example.com.zone")).unwrap()
    }

    #[test]
    fn test_exact_a_record() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("www.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::A);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert_eq!(resp.answers.len(), 2);
    }

    #[test]
    fn test_apex_a_record() {
        let zone = load_test_zone();
        let resp = resolve_query(&zone, &zone.origin, RecordType::A);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert_eq!(resp.answers.len(), 1);
    }

    #[test]
    fn test_cname_chase() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("ftp.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::A);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        // Should have CNAME + A records from www
        assert!(resp.answers.len() >= 2);
    }

    #[test]
    fn test_cname_chain() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("alias.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::A);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        // alias -> ftp -> www, should have CNAMEs + A records
        assert!(resp.answers.len() >= 3);
    }

    #[test]
    fn test_nxdomain() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("nonexistent.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::A);
        assert_eq!(resp.response_code, ResponseCode::NXDomain);
        assert!(resp.answers.is_empty());
        assert!(!resp.authority.is_empty());
    }

    #[test]
    fn test_nodata() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("www.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::MX);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert!(resp.answers.is_empty());
        assert!(!resp.authority.is_empty());
    }

    #[test]
    fn test_wildcard() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("anything.wild.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::A);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert_eq!(resp.answers.len(), 1);
        assert_eq!(resp.answers[0].name(), &qname);
    }

    #[test]
    fn test_wildcard_nodata() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("anything.wild.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::MX);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert!(resp.answers.is_empty());
    }

    #[test]
    fn test_mx_with_additional() {
        let zone = load_test_zone();
        let resp = resolve_query(&zone, &zone.origin, RecordType::MX);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert!(!resp.answers.is_empty());
        assert!(!resp.additional.is_empty());
    }

    #[test]
    fn test_txt_record() {
        let zone = load_test_zone();
        let qname = Name::from_ascii("info.example.com.").unwrap();
        let resp = resolve_query(&zone, &qname, RecordType::TXT);
        assert_eq!(resp.response_code, ResponseCode::NoError);
        assert_eq!(resp.answers.len(), 1);
    }
}
