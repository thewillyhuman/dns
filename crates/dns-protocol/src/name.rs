use hickory_proto::rr::Name;

/// Check if `name` is a subdomain of (or equal to) `zone`.
pub fn is_subdomain(name: &Name, zone: &Name) -> bool {
    if name.num_labels() < zone.num_labels() {
        return false;
    }
    // Compare from the rightmost labels
    let name_labels: Vec<_> = name.iter().rev().collect();
    let zone_labels: Vec<_> = zone.iter().rev().collect();
    for (n, z) in name_labels.iter().zip(zone_labels.iter()) {
        if !n.eq_ignore_ascii_case(z) {
            return false;
        }
    }
    true
}

/// Return the number of labels in `name` beyond `zone`.
/// Returns 0 if name == zone, 1 if name is a direct child, etc.
pub fn labels_below(name: &Name, zone: &Name) -> usize {
    if !is_subdomain(name, zone) {
        return 0;
    }
    (name.num_labels() - zone.num_labels()) as usize
}

/// Strip the leftmost label from a name, returning the parent.
pub fn parent(name: &Name) -> Option<Name> {
    if name.is_root() {
        return None;
    }
    let s = name.to_string();
    if let Some(pos) = s.find('.') {
        Name::from_ascii(&s[pos + 1..]).ok()
    } else {
        Some(Name::root())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_subdomain() {
        let zone = Name::from_ascii("example.com.").unwrap();
        let sub = Name::from_ascii("www.example.com.").unwrap();
        let other = Name::from_ascii("www.other.com.").unwrap();
        let exact = Name::from_ascii("example.com.").unwrap();

        assert!(is_subdomain(&sub, &zone));
        assert!(is_subdomain(&exact, &zone));
        assert!(!is_subdomain(&other, &zone));
        assert!(!is_subdomain(&zone, &sub));
    }

    #[test]
    fn test_labels_below() {
        let zone = Name::from_ascii("example.com.").unwrap();
        let exact = Name::from_ascii("example.com.").unwrap();
        let sub = Name::from_ascii("www.example.com.").unwrap();
        let deep = Name::from_ascii("a.b.example.com.").unwrap();

        assert_eq!(labels_below(&exact, &zone), 0);
        assert_eq!(labels_below(&sub, &zone), 1);
        assert_eq!(labels_below(&deep, &zone), 2);
    }

    #[test]
    fn test_parent() {
        let name = Name::from_ascii("www.example.com.").unwrap();
        let p = parent(&name).unwrap();
        assert_eq!(p, Name::from_ascii("example.com.").unwrap());

        let p2 = parent(&p).unwrap();
        assert_eq!(p2, Name::from_ascii("com.").unwrap());
    }
}
