use hickory_proto::rr::Name;

/// Apply QNAME minimization per RFC 9156.
/// Returns a minimized query name that reveals only one label
/// beyond the known zone cut.
///
/// For example, when resolving `www.example.com.` and we know
/// the nameserver is authoritative for `com.`, we send a query
/// for `example.com.` instead of `www.example.com.`.
pub fn minimized_qname(qname: &Name, zone_cut: &Name) -> Name {
    let qname_labels = qname.num_labels() as usize;
    let zone_labels = zone_cut.num_labels() as usize;

    if qname_labels <= zone_labels + 1 {
        // Already minimal or at the target
        return qname.clone();
    }

    // We want zone_labels + 1 labels from the right of qname
    let target_labels = zone_labels + 1;
    let skip = qname_labels - target_labels;

    // Build the minimized name by taking the rightmost target_labels labels
    let s = qname.to_ascii();
    let parts: Vec<&str> = s.trim_end_matches('.').split('.').collect();
    if skip >= parts.len() {
        return qname.clone();
    }

    let minimized = parts[skip..].join(".");
    Name::from_ascii(&format!("{}.", minimized)).unwrap_or_else(|_| qname.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimize_from_root() {
        let qname = Name::from_ascii("www.example.com.").unwrap();
        let zone_cut = Name::root();
        let minimized = minimized_qname(&qname, &zone_cut);
        assert_eq!(minimized, Name::from_ascii("com.").unwrap());
    }

    #[test]
    fn test_minimize_from_tld() {
        let qname = Name::from_ascii("www.example.com.").unwrap();
        let zone_cut = Name::from_ascii("com.").unwrap();
        let minimized = minimized_qname(&qname, &zone_cut);
        assert_eq!(minimized, Name::from_ascii("example.com.").unwrap());
    }

    #[test]
    fn test_minimize_at_target() {
        let qname = Name::from_ascii("www.example.com.").unwrap();
        let zone_cut = Name::from_ascii("example.com.").unwrap();
        let minimized = minimized_qname(&qname, &zone_cut);
        assert_eq!(minimized, Name::from_ascii("www.example.com.").unwrap());
    }

    #[test]
    fn test_minimize_already_minimal() {
        let qname = Name::from_ascii("com.").unwrap();
        let zone_cut = Name::root();
        let minimized = minimized_qname(&qname, &zone_cut);
        assert_eq!(minimized, Name::from_ascii("com.").unwrap());
    }

    #[test]
    fn test_minimize_deep() {
        let qname = Name::from_ascii("a.b.c.d.example.com.").unwrap();
        let zone_cut = Name::from_ascii("com.").unwrap();
        let minimized = minimized_qname(&qname, &zone_cut);
        assert_eq!(minimized, Name::from_ascii("example.com.").unwrap());
    }
}
