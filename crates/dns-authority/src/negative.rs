use crate::zone::Zone;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::{Name, Record};

/// Response from an authoritative lookup.
#[derive(Debug, Clone)]
pub struct AuthResponse {
    pub answers: Vec<Record>,
    pub authority: Vec<Record>,
    pub additional: Vec<Record>,
    pub response_code: ResponseCode,
}

impl AuthResponse {
    pub fn noerror(answers: Vec<Record>, authority: Vec<Record>, additional: Vec<Record>) -> Self {
        Self {
            answers,
            authority,
            additional,
            response_code: ResponseCode::NoError,
        }
    }
}

/// Build an NXDOMAIN response (name does not exist in the zone).
pub fn nxdomain_response(zone: &Zone) -> AuthResponse {
    AuthResponse {
        answers: Vec::new(),
        authority: vec![zone.soa.clone()],
        additional: Vec::new(),
        response_code: ResponseCode::NXDomain,
    }
}

/// Build a NODATA response (name exists but the requested type does not).
pub fn nodata_response(zone: &Zone, _qname: &Name) -> AuthResponse {
    AuthResponse {
        answers: Vec::new(),
        authority: vec![zone.soa.clone()],
        additional: Vec::new(),
        response_code: ResponseCode::NoError,
    }
}

/// Build a referral response (delegation to a child zone).
pub fn referral_response(ns_records: &[Record], glue: Vec<Record>) -> AuthResponse {
    AuthResponse {
        answers: Vec::new(),
        authority: ns_records.to_vec(),
        additional: glue,
        response_code: ResponseCode::NoError,
    }
}
