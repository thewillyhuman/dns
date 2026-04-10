use dns_authority::AuthResponse;
use hickory_proto::op::{Message, MessageType, ResponseCode};
use hickory_proto::rr::Record;

/// Build a DNS response message from an authoritative response.
pub fn build_authoritative_response(query: &Message, auth: &AuthResponse) -> Message {
    let mut response = Message::new();
    response.set_id(query.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(query.op_code());
    response.set_authoritative(true);
    response.set_recursion_desired(query.recursion_desired());
    response.set_recursion_available(false);
    response.set_response_code(auth.response_code);

    // Copy query section
    response.add_queries(query.queries().to_vec());

    // Add answer section
    response.add_answers(auth.answers.clone());

    // Add authority section
    response.add_name_servers(auth.authority.clone());

    // Add additional section
    response.add_additionals(auth.additional.clone());

    // Copy EDNS from query if present
    if let Some(edns) = query.extensions().as_ref() {
        let mut resp_edns = hickory_proto::op::Edns::new();
        resp_edns.set_max_payload(edns.max_payload().max(512));
        resp_edns.set_version(0);
        response.set_edns(resp_edns);
    }

    response
}

/// Build a REFUSED response.
pub fn build_refused(query: &Message) -> Message {
    let mut response = Message::new();
    response.set_id(query.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(query.op_code());
    response.set_recursion_desired(query.recursion_desired());
    response.set_response_code(ResponseCode::Refused);
    response.add_queries(query.queries().to_vec());
    response
}

/// Build a SERVFAIL response.
pub fn build_servfail(query: &Message) -> Message {
    let mut response = Message::new();
    response.set_id(query.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(query.op_code());
    response.set_recursion_desired(query.recursion_desired());
    response.set_response_code(ResponseCode::ServFail);
    response.add_queries(query.queries().to_vec());
    response
}

/// Build a FORMERR response (format error).
pub fn build_formerr(id: u16) -> Message {
    let mut response = Message::new();
    response.set_id(id);
    response.set_message_type(MessageType::Response);
    response.set_response_code(ResponseCode::FormErr);
    response
}

/// Build a recursive response (non-authoritative).
pub fn build_recursive_response(query: &Message, answers: Vec<Record>) -> Message {
    let mut response = Message::new();
    response.set_id(query.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(query.op_code());
    response.set_authoritative(false);
    response.set_recursion_desired(query.recursion_desired());
    response.set_recursion_available(true);
    response.set_response_code(ResponseCode::NoError);
    response.add_queries(query.queries().to_vec());
    response.add_answers(answers);
    response
}
