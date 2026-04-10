use crate::acl::AclEngine;
use crate::response;
use dns_authority::ZoneStore;
use dns_protocol::message;
use dns_resolver::Resolver;
use dns_transport::rate_limit::{RrlAction, RrlEngine};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, warn};

/// Central query router that dispatches to authoritative or recursive resolution.
pub struct Router {
    zone_store: Arc<ZoneStore>,
    resolver: Option<Arc<Resolver>>,
    acl: AclEngine,
    rrl: Option<Arc<RrlEngine>>,
}

impl Router {
    pub fn new(
        zone_store: Arc<ZoneStore>,
        resolver: Option<Arc<Resolver>>,
        acl: AclEngine,
    ) -> Self {
        Self {
            zone_store,
            resolver,
            acl,
            rrl: None,
        }
    }

    pub fn with_rrl(mut self, rrl: Arc<RrlEngine>) -> Self {
        self.rrl = Some(rrl);
        self
    }

    pub fn zone_store(&self) -> &Arc<ZoneStore> {
        &self.zone_store
    }
}

impl dns_transport::QueryHandler for Router {
    fn handle_query(
        &self,
        raw: &[u8],
        src: SocketAddr,
    ) -> impl std::future::Future<Output = Option<Vec<u8>>> + Send {
        let raw = raw.to_vec();
        let zone_store = Arc::clone(&self.zone_store);
        let resolver = self.resolver.clone();
        let acl = self.acl.clone();
        let rrl = self.rrl.clone();

        async move {
            // Parse the DNS message
            let query = match message::parse_message(&raw) {
                Ok(msg) => msg,
                Err(e) => {
                    debug!(src = %src, error = %e, "malformed query");
                    let id = if raw.len() >= 2 {
                        u16::from_be_bytes([raw[0], raw[1]])
                    } else {
                        0
                    };
                    let resp = response::build_formerr(id);
                    return resp.to_vec().ok();
                }
            };

            // Check query ACL
            if !acl.is_query_allowed(&src.ip()) {
                debug!(src = %src, "query refused by ACL");
                let resp = response::build_refused(&query);
                return resp.to_vec().ok();
            }

            let q = &query.queries()[0];
            let qname = q.name();
            let qtype = q.query_type();

            debug!(
                src = %src,
                qname = %qname,
                qtype = ?qtype,
                "processing query"
            );

            // Resolve the query
            let result = if zone_store.is_authoritative_for(qname) {
                if let Some(auth_response) = zone_store.lookup(qname, qtype) {
                    let resp = response::build_authoritative_response(&query, &auth_response);
                    resp.to_vec().ok()
                } else {
                    None
                }
            } else if query.recursion_desired() && acl.is_recursion_allowed(&src.ip()) {
                if let Some(resolver) = &resolver {
                    match resolver.resolve(qname, qtype).await {
                        Ok(resolve_resp) => {
                            let resp =
                                response::build_recursive_response(&query, resolve_resp.answers);
                            resp.to_vec().ok()
                        }
                        Err(e) => {
                            warn!(
                                src = %src,
                                qname = %qname,
                                error = %e,
                                "recursive resolution failed"
                            );
                            let resp = response::build_servfail(&query);
                            resp.to_vec().ok()
                        }
                    }
                } else {
                    warn!(src = %src, qname = %qname, "recursion requested but resolver not configured");
                    let resp = response::build_servfail(&query);
                    resp.to_vec().ok()
                }
            } else {
                let resp = response::build_refused(&query);
                resp.to_vec().ok()
            };

            // Apply RRL if enabled
            if let (Some(result), Some(rrl)) = (&result, &rrl) {
                match rrl.check(src.ip()) {
                    RrlAction::Allow => {}
                    RrlAction::Drop => {
                        debug!(src = %src, "RRL: dropping response");
                        return None;
                    }
                    RrlAction::Truncate => {
                        debug!(src = %src, "RRL: truncating response");
                        // Set TC bit in the response
                        if result.len() >= 4 {
                            let mut truncated = result[..12.min(result.len())].to_vec();
                            truncated[2] |= 0x02; // Set TC bit
                            return Some(truncated);
                        }
                    }
                }
            }

            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dns_authority::loader::parse_zone_str;
    use dns_transport::QueryHandler;
    use hickory_proto::op::{Message, MessageType, OpCode, Query};
    use hickory_proto::rr::{DNSClass, Name, RecordType};
    use std::collections::HashMap;
    use std::path::PathBuf;

    const TEST_ZONE: &str = r#"
$ORIGIN example.com.
$TTL 3600
@   IN  SOA ns1.example.com. admin.example.com. (
            2024010101 3600 900 604800 86400 )
    IN  NS  ns1.example.com.
    IN  A   192.0.2.1
ns1 IN  A   192.0.2.10
www IN  A   192.0.2.100
"#;

    fn test_router() -> Router {
        let zone = parse_zone_str(TEST_ZONE, &PathBuf::from("example.com.zone")).unwrap();
        let mut zones = HashMap::new();
        zones.insert(zone.origin.clone(), zone);
        let store = Arc::new(ZoneStore::new(zones));
        let acl = AclEngine::new(HashMap::new(), "any", "any");
        Router::new(store, None, acl)
    }

    fn build_query_msg(name: &str, qtype: RecordType) -> Vec<u8> {
        let mut msg = Message::new();
        msg.set_id(0xABCD);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(false);
        let mut query = Query::new();
        query.set_name(Name::from_ascii(name).unwrap());
        query.set_query_type(qtype);
        query.set_query_class(DNSClass::IN);
        msg.add_query(query);
        msg.to_vec().unwrap()
    }

    #[tokio::test]
    async fn test_authoritative_query() {
        let router = test_router();
        let raw = build_query_msg("www.example.com.", RecordType::A);
        let src: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let resp_bytes = router.handle_query(&raw, src).await.unwrap();
        let resp = Message::from_vec(&resp_bytes).unwrap();
        assert!(resp.authoritative());
        assert_eq!(resp.answers().len(), 1);
    }

    #[tokio::test]
    async fn test_nxdomain_query() {
        let router = test_router();
        let raw = build_query_msg("nonexistent.example.com.", RecordType::A);
        let src: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let resp_bytes = router.handle_query(&raw, src).await.unwrap();
        let resp = Message::from_vec(&resp_bytes).unwrap();
        assert!(resp.authoritative());
        assert_eq!(
            resp.response_code(),
            hickory_proto::op::ResponseCode::NXDomain
        );
    }

    #[tokio::test]
    async fn test_non_authoritative_refused() {
        let router = test_router();
        let raw = build_query_msg("www.google.com.", RecordType::A);
        let src: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let resp_bytes = router.handle_query(&raw, src).await.unwrap();
        let resp = Message::from_vec(&resp_bytes).unwrap();
        assert_eq!(
            resp.response_code(),
            hickory_proto::op::ResponseCode::Refused
        );
    }
}
