use crate::cache::DnsCache;
use crate::dedup::{DedupAction, DedupMap, DedupResult};
use crate::qname_min;
use crate::upstream::{UpstreamConfig, UpstreamPool};
use dns_config::config::{CacheConfig, RecursionConfig};
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("maximum recursion depth exceeded")]
    MaxDepthExceeded,
    #[error("upstream error: {0}")]
    Upstream(#[from] crate::upstream::UpstreamError),
    #[error("no nameservers available for {0}")]
    NoNameservers(Name),
    #[error("resolution failed: SERVFAIL")]
    ServFail,
}

/// Response from the recursive resolver.
#[derive(Debug, Clone)]
pub struct ResolveResponse {
    pub answers: Vec<Record>,
    pub authority: Vec<Record>,
    pub additional: Vec<Record>,
    pub response_code: ResponseCode,
}

/// Root hints — the starting nameservers for iterative resolution.
#[derive(Debug, Clone)]
pub struct RootHints {
    pub servers: Vec<SocketAddr>,
}

impl Default for RootHints {
    fn default() -> Self {
        // Default root server addresses
        Self {
            servers: vec![
                "198.41.0.4:53".parse().unwrap(),     // a.root-servers.net
                "170.247.170.2:53".parse().unwrap(),  // b.root-servers.net
                "192.33.4.12:53".parse().unwrap(),    // c.root-servers.net
                "199.7.91.13:53".parse().unwrap(),    // d.root-servers.net
                "192.203.230.10:53".parse().unwrap(), // e.root-servers.net
                "192.5.5.241:53".parse().unwrap(),    // f.root-servers.net
                "192.112.36.4:53".parse().unwrap(),   // g.root-servers.net
                "198.97.190.53:53".parse().unwrap(),  // h.root-servers.net
                "192.36.148.17:53".parse().unwrap(),  // i.root-servers.net
                "192.58.128.30:53".parse().unwrap(),  // j.root-servers.net
                "193.0.14.129:53".parse().unwrap(),   // k.root-servers.net
                "199.7.83.42:53".parse().unwrap(),    // l.root-servers.net
                "202.12.27.33:53".parse().unwrap(),   // m.root-servers.net
            ],
        }
    }
}

/// Recursive DNS resolver.
pub struct Resolver {
    cache: Arc<DnsCache>,
    dedup: DedupMap,
    upstream: UpstreamPool,
    root_hints: RootHints,
    max_depth: u32,
    qname_minimization: bool,
    forwarding: std::collections::HashMap<String, Vec<SocketAddr>>,
}

impl Resolver {
    pub fn new(recursion_config: &RecursionConfig, cache_config: &CacheConfig) -> Self {
        let cache = Arc::new(DnsCache::new(
            cache_config.max_entries,
            cache_config.min_ttl,
            cache_config.max_ttl,
            cache_config.negative_ttl,
        ));

        let timeout =
            parse_duration(&recursion_config.timeout).unwrap_or(std::time::Duration::from_secs(2));
        let upstream = UpstreamPool::new(UpstreamConfig {
            timeout,
            retries: recursion_config.retries,
        });

        Self {
            cache,
            dedup: DedupMap::new(),
            upstream,
            root_hints: RootHints::default(),
            max_depth: recursion_config.max_depth,
            qname_minimization: recursion_config.qname_minimization,
            forwarding: recursion_config.forwarding.zones.clone(),
        }
    }

    /// Resolve a DNS query recursively.
    pub async fn resolve(
        &self,
        qname: &Name,
        qtype: RecordType,
    ) -> Result<ResolveResponse, ResolveError> {
        // Check cache first
        if let Some(cached) = self.cache.get(qname, qtype) {
            debug!(qname = %qname, qtype = ?qtype, "cache hit");
            return Ok(ResolveResponse {
                answers: cached.records,
                authority: Vec::new(),
                additional: Vec::new(),
                response_code: cached.response_code,
            });
        }

        // Check for forwarding or iterative resolution
        let forwarders = self.find_forwarders(qname);

        // Deduplication — applies to both forwarded and iterative queries
        match self.dedup.try_dedup(qname, qtype) {
            DedupAction::Execute(guard) => {
                let result = if let Some(fwd) = forwarders {
                    self.forward_query(qname, qtype, &fwd).await
                } else {
                    self.iterative_resolve(qname, qtype, 0).await
                };
                match &result {
                    Ok(resp) => {
                        // Cache the result
                        if !resp.answers.is_empty() {
                            let ttl = resp.answers.first().map(|r| r.ttl()).unwrap_or(300);
                            self.cache.insert(qname, qtype, resp.answers.clone(), ttl);
                        } else if resp.response_code == ResponseCode::NXDomain {
                            self.cache
                                .insert_negative(qname, qtype, ResponseCode::NXDomain);
                        }
                        guard.complete(DedupResult {
                            records: resp.answers.clone(),
                            response_code: resp.response_code,
                        });
                    }
                    Err(_) => {
                        // Guard drops automatically, removing the entry
                    }
                }
                result
            }
            DedupAction::Wait(mut rx) => match rx.recv().await {
                Ok(result) => Ok(ResolveResponse {
                    answers: result.records,
                    authority: Vec::new(),
                    additional: Vec::new(),
                    response_code: result.response_code,
                }),
                Err(_) => Err(ResolveError::ServFail),
            },
        }
    }

    /// Iterative resolution starting from root hints.
    fn iterative_resolve<'a>(
        &'a self,
        qname: &'a Name,
        qtype: RecordType,
        depth: u32,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ResolveResponse, ResolveError>> + Send + 'a>,
    > {
        Box::pin(async move {
            if depth >= self.max_depth {
                warn!(qname = %qname, depth = depth, "max recursion depth exceeded");
                return Err(ResolveError::MaxDepthExceeded);
            }

            // Start from root hints or cached delegation
            let nameservers = self.root_hints.servers.clone();
            let zone_cut = Name::root();

            // Try to find a closer delegation in cache
            // Walk from qname toward root, looking for cached NS
            // (simplified — a full implementation would check NS record cache)

            let actual_qname = if self.qname_minimization {
                qname_min::minimized_qname(qname, &zone_cut)
            } else {
                qname.clone()
            };

            // Query each nameserver until we get an answer or referral
            for ns_addr in &nameservers {
                let response = match self.upstream.query(*ns_addr, &actual_qname, qtype).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        debug!(server = %ns_addr, error = %e, "upstream query failed, trying next");
                        continue;
                    }
                };

                return self
                    .process_response(response, qname, qtype, &zone_cut, depth)
                    .await;
            }

            Err(ResolveError::NoNameservers(zone_cut))
        }) // end Box::pin
    }

    /// Process an upstream response: answer, referral, or error.
    fn process_response<'a>(
        &'a self,
        response: Message,
        qname: &'a Name,
        qtype: RecordType,
        zone_cut: &'a Name,
        depth: u32,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ResolveResponse, ResolveError>> + Send + 'a>,
    > {
        Box::pin(async move {
            let rcode = response.response_code();

            // Check for answer
            if !response.answers().is_empty() {
                let answers = response.answers().to_vec();

                // Check for CNAME and chase if needed
                let cname_target: Option<Name> = if qtype != RecordType::CNAME {
                    answers
                        .iter()
                        .find(|r| r.record_type() == RecordType::CNAME)
                        .and_then(|r| {
                            if let RData::CNAME(cname) = r.data() {
                                Some(cname.0.clone())
                            } else {
                                None
                            }
                        })
                } else {
                    None
                };

                if let Some(target) = cname_target {
                    // Check if the answer already contains the final records
                    let has_final = answers.iter().any(|r| r.record_type() == qtype);
                    if has_final {
                        return Ok(ResolveResponse {
                            answers,
                            authority: response.name_servers().to_vec(),
                            additional: response.additionals().to_vec(),
                            response_code: ResponseCode::NoError,
                        });
                    }
                    // Chase the CNAME
                    let mut cname_answers = answers;
                    let chased = self.iterative_resolve(&target, qtype, depth + 1).await?;
                    cname_answers.extend(chased.answers);
                    return Ok(ResolveResponse {
                        answers: cname_answers,
                        authority: chased.authority,
                        additional: chased.additional,
                        response_code: chased.response_code,
                    });
                }

                return Ok(ResolveResponse {
                    answers,
                    authority: response.name_servers().to_vec(),
                    additional: response.additionals().to_vec(),
                    response_code: rcode,
                });
            }

            // Check for referral (NS records in authority section, no answers)
            if !response.name_servers().is_empty()
                && rcode == ResponseCode::NoError
                && response.answers().is_empty()
            {
                let ns_records = response.name_servers();
                let additionals = response.additionals();

                // Extract NS targets and their glue records
                let mut referred_servers = Vec::new();
                let mut new_zone_cut = zone_cut.clone();

                for ns in ns_records {
                    if ns.record_type() == RecordType::NS {
                        if let RData::NS(ns_name) = ns.data() {
                            // Update zone cut
                            new_zone_cut = ns.name().clone();

                            // Look for glue A/AAAA records
                            for additional in additionals {
                                if additional.name() == &ns_name.0 {
                                    match additional.data() {
                                        RData::A(a) => {
                                            referred_servers.push(SocketAddr::new(
                                                std::net::IpAddr::V4(a.0),
                                                53,
                                            ));
                                        }
                                        RData::AAAA(aaaa) => {
                                            referred_servers.push(SocketAddr::new(
                                                std::net::IpAddr::V6(aaaa.0),
                                                53,
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }

                if referred_servers.is_empty() {
                    // No glue — need to resolve NS names first
                    // For simplicity, try first NS name
                    for ns in ns_records {
                        if ns.record_type() == RecordType::NS {
                            if let RData::NS(ns_name) = ns.data() {
                                if let Ok(ns_resp) = self
                                    .iterative_resolve(&ns_name.0, RecordType::A, depth + 1)
                                    .await
                                {
                                    for record in &ns_resp.answers {
                                        if let RData::A(a) = record.data() {
                                            referred_servers.push(SocketAddr::new(
                                                std::net::IpAddr::V4(a.0),
                                                53,
                                            ));
                                        }
                                    }
                                    if !referred_servers.is_empty() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                if referred_servers.is_empty() {
                    return Err(ResolveError::NoNameservers(new_zone_cut));
                }

                // Follow referral
                let actual_qname = if self.qname_minimization {
                    qname_min::minimized_qname(qname, &new_zone_cut)
                } else {
                    qname.clone()
                };

                for ns_addr in &referred_servers {
                    let response = match self.upstream.query(*ns_addr, &actual_qname, qtype).await {
                        Ok(resp) => resp,
                        Err(_) => continue,
                    };

                    return self
                        .process_response(response, qname, qtype, &new_zone_cut, depth + 1)
                        .await;
                }

                return Err(ResolveError::NoNameservers(new_zone_cut));
            }

            // NXDOMAIN or other error
            Ok(ResolveResponse {
                answers: Vec::new(),
                authority: response.name_servers().to_vec(),
                additional: Vec::new(),
                response_code: rcode,
            })
        }) // end Box::pin
    }

    /// Forward a query to configured forwarders.
    async fn forward_query(
        &self,
        qname: &Name,
        qtype: RecordType,
        forwarders: &[SocketAddr],
    ) -> Result<ResolveResponse, ResolveError> {
        for server in forwarders {
            match self.upstream.query(*server, qname, qtype).await {
                Ok(response) => {
                    return Ok(ResolveResponse {
                        answers: response.answers().to_vec(),
                        authority: response.name_servers().to_vec(),
                        additional: response.additionals().to_vec(),
                        response_code: response.response_code(),
                    });
                }
                Err(e) => {
                    warn!(server = %server, error = %e, "forwarder query failed");
                    continue;
                }
            }
        }
        Err(ResolveError::ServFail)
    }

    fn find_forwarders(&self, qname: &Name) -> Option<Vec<SocketAddr>> {
        // Check for exact zone match first
        let qname_str = qname.to_ascii();
        if let Some(servers) = self.forwarding.get(&qname_str) {
            return Some(servers.clone());
        }

        // Check for wildcard forwarder ("." forwards everything)
        if let Some(servers) = self.forwarding.get(".") {
            return Some(servers.clone());
        }

        None
    }

    /// Flush the entire cache.
    pub fn flush_cache(&self) {
        self.cache.flush_all();
    }

    /// Flush a specific name from cache.
    pub fn flush_name(&self, name: &Name) {
        self.cache.flush_name(name);
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats()
    }

    /// Get the number of in-flight queries.
    pub fn inflight_count(&self) -> usize {
        self.dedup.inflight_count()
    }
}

fn parse_duration(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        ms.parse::<u64>().ok().map(std::time::Duration::from_millis)
    } else if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<u64>().ok().map(std::time::Duration::from_secs)
    } else {
        s.parse::<u64>().ok().map(std::time::Duration::from_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(
            parse_duration("2s"),
            Some(std::time::Duration::from_secs(2))
        );
        assert_eq!(
            parse_duration("500ms"),
            Some(std::time::Duration::from_millis(500))
        );
        assert_eq!(
            parse_duration("10"),
            Some(std::time::Duration::from_secs(10))
        );
    }

    #[test]
    fn test_root_hints() {
        let hints = RootHints::default();
        assert_eq!(hints.servers.len(), 13);
    }
}
