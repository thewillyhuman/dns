use hickory_proto::op::Edns;
use hickory_proto::op::Message;

/// Extract EDNS information from a DNS message, if present.
pub fn get_edns(msg: &Message) -> Option<&Edns> {
    msg.extensions().as_ref()
}

/// Get the maximum UDP payload size advertised by EDNS, or 512 if no EDNS.
pub fn max_udp_payload(msg: &Message) -> u16 {
    msg.extensions()
        .as_ref()
        .map(|edns| edns.max_payload())
        .unwrap_or(512)
}

/// Check if the DNSSEC OK (DO) bit is set in the EDNS flags.
pub fn dnssec_ok(msg: &Message) -> bool {
    msg.extensions()
        .as_ref()
        .map(|edns| edns.flags().dnssec_ok)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_edns() {
        let msg = Message::new();
        assert!(get_edns(&msg).is_none());
        assert_eq!(max_udp_payload(&msg), 512);
        assert!(!dnssec_ok(&msg));
    }
}
