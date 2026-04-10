use hickory_proto::op::Message;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("failed to parse DNS message: {0}")]
    Parse(#[from] hickory_proto::ProtoError),
    #[error("invalid query count: expected 1, got {0}")]
    InvalidQueryCount(u16),
    #[error("message too large: {size} bytes exceeds limit of {limit}")]
    TooLarge { size: usize, limit: usize },
}

/// Parse a DNS message from wire format bytes.
/// Validates that QDCOUNT == 1 per spec requirement.
pub fn parse_message(bytes: &[u8]) -> Result<Message, MessageError> {
    let msg = Message::from_vec(bytes)?;
    let qcount = msg.query_count();
    if qcount != 1 {
        return Err(MessageError::InvalidQueryCount(qcount));
    }
    Ok(msg)
}

/// Serialize a DNS message to wire format bytes.
pub fn serialize_message(msg: &Message) -> Result<Vec<u8>, MessageError> {
    Ok(msg.to_vec()?)
}

/// Serialize a DNS message, truncating if it exceeds max_size.
/// Sets the TC (truncation) bit if the message was truncated.
pub fn serialize_with_limit(msg: &Message, max_size: usize) -> Result<Vec<u8>, MessageError> {
    let bytes = msg.to_vec()?;
    if bytes.len() <= max_size {
        return Ok(bytes);
    }

    // Build a truncated response: header + question only, TC=1
    let mut truncated = Message::new();
    truncated.set_id(msg.id());
    truncated.set_message_type(msg.message_type());
    truncated.set_op_code(msg.op_code());
    truncated.set_authoritative(msg.authoritative());
    truncated.set_truncated(true);
    truncated.set_recursion_desired(msg.recursion_desired());
    truncated.set_recursion_available(msg.recursion_available());
    truncated.set_response_code(msg.response_code());
    truncated.add_queries(msg.queries().to_vec());

    Ok(truncated.to_vec()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::op::{MessageType, OpCode, Query};
    use hickory_proto::rr::{DNSClass, Name, RecordType};

    fn build_query() -> Message {
        let mut msg = Message::new();
        msg.set_id(0x1234);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(true);
        let mut query = Query::new();
        query.set_name(Name::from_ascii("example.com.").unwrap());
        query.set_query_type(RecordType::A);
        query.set_query_class(DNSClass::IN);
        msg.add_query(query);
        msg
    }

    #[test]
    fn test_round_trip() {
        let msg = build_query();
        let bytes = serialize_message(&msg).unwrap();
        let parsed = parse_message(&bytes).unwrap();
        assert_eq!(parsed.id(), 0x1234);
        assert_eq!(parsed.queries().len(), 1);
        assert_eq!(
            parsed.queries()[0].name(),
            &Name::from_ascii("example.com.").unwrap()
        );
    }

    #[test]
    fn test_truncation() {
        let msg = build_query();
        // Force truncation with a very small limit
        let bytes = serialize_with_limit(&msg, 12).unwrap();
        let parsed = Message::from_vec(&bytes).unwrap();
        assert!(parsed.truncated());
    }
}
