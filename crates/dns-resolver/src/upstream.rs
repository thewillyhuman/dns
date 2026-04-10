use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{DNSClass, Name, RecordType};
use rand::Rng;
use std::net::SocketAddr;
use std::time::Duration;
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum UpstreamError {
    #[error("upstream query timed out after {0:?}")]
    Timeout(Duration),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Proto(#[from] hickory_proto::ProtoError),
    #[error("response ID mismatch: expected {expected}, got {got}")]
    IdMismatch { expected: u16, got: u16 },
    #[error("all retries exhausted")]
    RetriesExhausted,
}

/// Configuration for upstream queries.
#[derive(Debug, Clone)]
pub struct UpstreamConfig {
    pub timeout: Duration,
    pub retries: u32,
}

impl Default for UpstreamConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(2),
            retries: 2,
        }
    }
}

/// Pool for sending queries to upstream DNS servers.
pub struct UpstreamPool {
    config: UpstreamConfig,
}

impl UpstreamPool {
    pub fn new(config: UpstreamConfig) -> Self {
        Self { config }
    }

    /// Send a query to an upstream server with retries.
    pub async fn query(
        &self,
        server: SocketAddr,
        qname: &Name,
        qtype: RecordType,
    ) -> Result<Message, UpstreamError> {
        let mut last_error = None;

        for attempt in 0..=self.config.retries {
            if attempt > 0 {
                debug!(server = %server, attempt = attempt, "retrying upstream query");
            }

            match self.query_once(server, qname, qtype).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    warn!(
                        server = %server,
                        qname = %qname,
                        error = %e,
                        attempt = attempt,
                        "upstream query failed"
                    );
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or(UpstreamError::RetriesExhausted))
    }

    async fn query_once(
        &self,
        server: SocketAddr,
        qname: &Name,
        qtype: RecordType,
    ) -> Result<Message, UpstreamError> {
        // Build query message
        let mut msg = Message::new();
        let id: u16 = rand::thread_rng().gen();
        msg.set_id(id);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(false); // We're doing iterative resolution

        // 0x20 mixed-case encoding for additional entropy
        let mixed_name = apply_0x20_encoding(qname);
        let mut query = Query::new();
        query.set_name(mixed_name.clone());
        query.set_query_type(qtype);
        query.set_query_class(DNSClass::IN);
        msg.add_query(query);

        let wire = msg.to_vec()?;

        // Bind to random port for source port randomization
        let bind_addr: SocketAddr = if server.is_ipv4() {
            "0.0.0.0:0".parse().unwrap()
        } else {
            "[::]:0".parse().unwrap()
        };
        let socket = UdpSocket::bind(bind_addr).await?;

        socket.send_to(&wire, server).await?;

        // Receive response with timeout
        let mut buf = vec![0u8; 4096];
        let (len, _from) = timeout(self.config.timeout, socket.recv_from(&mut buf))
            .await
            .map_err(|_| UpstreamError::Timeout(self.config.timeout))??;

        let response = Message::from_vec(&buf[..len])?;

        // Validate response
        if response.id() != id {
            return Err(UpstreamError::IdMismatch {
                expected: id,
                got: response.id(),
            });
        }

        // Verify 0x20 encoding: response should preserve query name case
        if let Some(resp_query) = response.queries().first() {
            if resp_query.name().to_ascii() != mixed_name.to_ascii() {
                debug!(
                    expected = %mixed_name,
                    got = %resp_query.name(),
                    "0x20 case mismatch in response (non-compliant server)"
                );
                // Don't reject — many servers don't preserve case
            }
        }

        // TODO: TCP fallback on truncation (TC bit)
        if response.truncated() {
            debug!(server = %server, "response truncated, TCP fallback needed");
            // For now, return the truncated response; TCP fallback in iteration 7
        }

        Ok(response)
    }
}

/// Apply 0x20 mixed-case encoding to a domain name.
/// Randomizes the case of ASCII letters for additional entropy.
fn apply_0x20_encoding(name: &Name) -> Name {
    let mut rng = rand::thread_rng();
    let s = name.to_ascii();
    let mixed: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                if rng.gen() {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            } else {
                c
            }
        })
        .collect();
    Name::from_ascii(&mixed).unwrap_or_else(|_| name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0x20_encoding() {
        let name = Name::from_ascii("example.com.").unwrap();
        let mixed = apply_0x20_encoding(&name);
        // Should be the same name (case-insensitive)
        assert_eq!(
            name.to_ascii().to_lowercase(),
            mixed.to_ascii().to_lowercase()
        );
    }

    #[test]
    fn test_upstream_config_default() {
        let config = UpstreamConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(2));
        assert_eq!(config.retries, 2);
    }
}
