// Re-export hickory opcodes and response codes with convenience methods.

pub use hickory_proto::op::{OpCode, ResponseCode};

/// Check if a response code indicates an error.
pub fn is_error(rcode: ResponseCode) -> bool {
    !matches!(rcode, ResponseCode::NoError)
}

/// Check if a response code indicates a name error (NXDOMAIN).
pub fn is_nxdomain(rcode: ResponseCode) -> bool {
    matches!(rcode, ResponseCode::NXDomain)
}

/// Check if a response code indicates a server failure.
pub fn is_servfail(rcode: ResponseCode) -> bool {
    matches!(rcode, ResponseCode::ServFail)
}
