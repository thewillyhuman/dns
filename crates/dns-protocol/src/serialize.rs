use thiserror::Error;

#[derive(Debug, Error)]
pub enum SerializeError {
    #[error("serialization error: {0}")]
    Proto(#[from] hickory_proto::ProtoError),
}

/// Trait for types that can be serialized to/from DNS wire format.
pub trait WireFormat: Sized {
    fn to_wire(&self) -> Result<Vec<u8>, SerializeError>;
    fn from_wire(data: &[u8]) -> Result<Self, SerializeError>;
}

impl WireFormat for hickory_proto::op::Message {
    fn to_wire(&self) -> Result<Vec<u8>, SerializeError> {
        Ok(self.to_vec()?)
    }

    fn from_wire(data: &[u8]) -> Result<Self, SerializeError> {
        Ok(Self::from_vec(data)?)
    }
}
