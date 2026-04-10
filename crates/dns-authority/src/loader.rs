use crate::zone::Zone;
use hickory_proto::rr::{Name, RecordType};
use hickory_proto::serialize::txt::Parser;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ZoneLoadError {
    #[error("failed to read zone file {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse zone file {path}: {reason}")]
    Parse { path: String, reason: String },
    #[error("zone file {path} has no SOA record")]
    MissingSoa { path: String },
    #[error("failed to read zone directory: {0}")]
    DirRead(#[from] std::io::Error),
}

/// Load a single zone file from disk.
pub fn load_zone_file(path: &Path) -> Result<Zone, ZoneLoadError> {
    let content = std::fs::read_to_string(path).map_err(|e| ZoneLoadError::Io {
        path: path.display().to_string(),
        source: e,
    })?;

    parse_zone_str(&content, path)
}

/// Parse a zone from a string (for testing).
pub fn parse_zone_str(content: &str, path: &Path) -> Result<Zone, ZoneLoadError> {
    // Try to extract $ORIGIN from the file
    let origin = extract_origin(content).unwrap_or_else(|| {
        // Fall back to deriving origin from filename
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        Name::from_ascii(&format!("{}.", stem)).unwrap_or_else(|_| Name::root())
    });

    let parser = Parser::new(content.to_string(), None, Some(origin.clone()));
    let (final_origin, record_sets) =
        parser.parse().map_err(|e| ZoneLoadError::Parse {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;

    // Find SOA record
    let soa = record_sets
        .values()
        .find(|rs| rs.record_type() == RecordType::SOA && rs.name() == &final_origin)
        .and_then(|rs| rs.records_without_rrsigs().next().cloned())
        .ok_or_else(|| ZoneLoadError::MissingSoa {
            path: path.display().to_string(),
        })?;

    let mut zone = Zone::new(final_origin.clone(), soa);

    for record_set in record_sets.values() {
        for record in record_set.records_without_rrsigs() {
            // Skip the SOA at apex since Zone::new already adds it
            if record.record_type() == RecordType::SOA && record.name() == &final_origin {
                continue;
            }
            zone.add_record(record.clone());
        }
    }

    tracing::info!(
        zone = %zone.origin,
        records = zone.record_count(),
        serial = zone.serial(),
        "loaded zone"
    );

    Ok(zone)
}

/// Load all zone files from a directory.
pub fn load_zone_directory(dir: &Path) -> Result<HashMap<Name, Zone>, ZoneLoadError> {
    let mut zones = HashMap::new();

    if !dir.exists() {
        tracing::warn!(dir = %dir.display(), "zone directory does not exist");
        return Ok(zones);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "zone" || ext == "db" {
                match load_zone_file(&path) {
                    Ok(zone) => {
                        zones.insert(zone.origin.clone(), zone);
                    }
                    Err(e) => {
                        tracing::error!(path = %path.display(), error = %e, "failed to load zone file");
                    }
                }
            }
        }
    }

    tracing::info!(count = zones.len(), "loaded zones from directory");
    Ok(zones)
}

fn extract_origin(content: &str) -> Option<Name> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("$ORIGIN") {
            let origin_str = rest.trim().trim_end_matches(';');
            let origin_str = origin_str.split_whitespace().next()?;
            return Name::from_ascii(origin_str).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const TEST_ZONE: &str = r#"
$ORIGIN example.com.
$TTL 3600

@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101  ; serial
            3600        ; refresh
            900         ; retry
            604800      ; expire
            86400       ; minimum TTL
        )

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

*.wild  IN  A   192.0.2.200

info    IN  TXT "Example zone for testing"
"#;

    #[test]
    fn test_parse_zone() {
        let zone = parse_zone_str(TEST_ZONE, &PathBuf::from("example.com.zone")).unwrap();
        assert_eq!(zone.origin, Name::from_ascii("example.com.").unwrap());
        assert_eq!(zone.serial(), 2024010101);
        assert!(zone.record_count() > 5);
    }

    #[test]
    fn test_lookup_after_load() {
        let zone = parse_zone_str(TEST_ZONE, &PathBuf::from("example.com.zone")).unwrap();

        let www = Name::from_ascii("www.example.com.").unwrap();
        let a_records = zone.lookup_exact(&www, RecordType::A);
        assert!(a_records.is_some());
        assert_eq!(a_records.unwrap().len(), 2);

        let ftp = Name::from_ascii("ftp.example.com.").unwrap();
        let cname_records = zone.lookup_exact(&ftp, RecordType::CNAME);
        assert!(cname_records.is_some());
    }

    #[test]
    fn test_missing_soa() {
        let content = r#"
$ORIGIN example.com.
$TTL 3600
www IN A 192.0.2.1
"#;
        let result = parse_zone_str(content, &PathBuf::from("test.zone"));
        assert!(result.is_err());
    }

    #[test]
    fn test_wildcard_in_zone() {
        let zone = parse_zone_str(TEST_ZONE, &PathBuf::from("example.com.zone")).unwrap();
        let wild = Name::from_ascii("*.wild.example.com.").unwrap();
        assert!(zone.name_exists(&wild));
    }

    #[test]
    fn test_load_zone_directory() {
        // Cargo runs tests from the workspace root
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let workspace_root = Path::new(manifest_dir).parent().unwrap().parent().unwrap();
        let zones_dir = workspace_root.join("config/zones");
        let zones = load_zone_directory(&zones_dir).unwrap();
        assert!(!zones.is_empty());
    }
}
