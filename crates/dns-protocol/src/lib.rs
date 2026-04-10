pub mod edns;
pub mod message;
pub mod name;
pub mod opcode;
pub mod record;
pub mod serialize;

// Re-export core hickory types for convenience
pub use hickory_proto::op::{Header, Message, MessageType, OpCode, Query, ResponseCode};
pub use hickory_proto::rr::{DNSClass, Name, RData, Record, RecordType};
