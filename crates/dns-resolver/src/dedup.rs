use dashmap::DashMap;
use hickory_proto::rr::{Name, RecordType};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Key for deduplicating in-flight queries.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DedupKey {
    name: Name,
    rtype: RecordType,
}

/// Result of a deduplicated query.
#[derive(Debug, Clone)]
pub struct DedupResult {
    pub records: Vec<hickory_proto::rr::Record>,
    pub response_code: hickory_proto::op::ResponseCode,
}

/// In-flight query deduplication map.
/// If multiple clients ask for the same name+type simultaneously,
/// only one upstream query is performed and all waiters get the result.
pub struct DedupMap {
    inflight: DashMap<DedupKey, Arc<broadcast::Sender<DedupResult>>>,
}

impl DedupMap {
    pub fn new() -> Self {
        Self {
            inflight: DashMap::new(),
        }
    }

    /// Try to register an in-flight query. Returns:
    /// - `DedupAction::Execute(guard)` if this is the first query for this key. The caller
    ///   should perform the resolution and call `guard.complete(result)`.
    /// - `DedupAction::Wait(receiver)` if another query is already in-flight. The caller
    ///   should await the receiver.
    pub fn try_dedup(&self, name: &Name, rtype: RecordType) -> DedupAction<'_> {
        let key = DedupKey {
            name: name.clone(),
            rtype,
        };

        // Check if already in-flight
        if let Some(sender) = self.inflight.get(&key) {
            let rx = sender.subscribe();
            return DedupAction::Wait(rx);
        }

        // Register as the first query
        let (tx, _) = broadcast::channel(1);
        let tx = Arc::new(tx);
        self.inflight.insert(key.clone(), Arc::clone(&tx));

        DedupAction::Execute(DedupGuard {
            key,
            sender: tx,
            map: &self.inflight,
        })
    }

    /// Number of currently in-flight queries.
    pub fn inflight_count(&self) -> usize {
        self.inflight.len()
    }
}

impl Default for DedupMap {
    fn default() -> Self {
        Self::new()
    }
}

pub enum DedupAction<'a> {
    /// This is the first query — execute it and complete the guard.
    Execute(DedupGuard<'a>),
    /// Another query is in-flight — wait for the result.
    Wait(broadcast::Receiver<DedupResult>),
}

/// Guard that must be completed with the resolution result.
/// On drop without completion, removes the entry so future queries can retry.
pub struct DedupGuard<'a> {
    key: DedupKey,
    sender: Arc<broadcast::Sender<DedupResult>>,
    map: &'a DashMap<DedupKey, Arc<broadcast::Sender<DedupResult>>>,
}

impl<'a> DedupGuard<'a> {
    /// Complete the in-flight query with a result, notifying all waiters.
    pub fn complete(self, result: DedupResult) {
        let _ = self.sender.send(result);
        self.map.remove(&self.key);
        // Prevent Drop from running
        std::mem::forget(self);
    }
}

impl<'a> Drop for DedupGuard<'a> {
    fn drop(&mut self) {
        // Remove from map so future queries can retry
        self.map.remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::op::ResponseCode;

    #[tokio::test]
    async fn test_first_query_executes() {
        let map = DedupMap::new();
        let name = Name::from_ascii("example.com.").unwrap();

        match map.try_dedup(&name, RecordType::A) {
            DedupAction::Execute(guard) => {
                guard.complete(DedupResult {
                    records: Vec::new(),
                    response_code: ResponseCode::NoError,
                });
            }
            DedupAction::Wait(_) => panic!("expected Execute"),
        }

        assert_eq!(map.inflight_count(), 0);
    }

    #[tokio::test]
    async fn test_dedup_waiter() {
        let map = DedupMap::new();
        let name = Name::from_ascii("example.com.").unwrap();

        // First query
        let guard = match map.try_dedup(&name, RecordType::A) {
            DedupAction::Execute(g) => g,
            DedupAction::Wait(_) => panic!("expected Execute"),
        };

        // Second query should wait
        let mut rx = match map.try_dedup(&name, RecordType::A) {
            DedupAction::Wait(rx) => rx,
            DedupAction::Execute(_) => panic!("expected Wait"),
        };

        // Complete the first query
        guard.complete(DedupResult {
            records: Vec::new(),
            response_code: ResponseCode::NoError,
        });

        // Waiter should receive the result
        let result = rx.recv().await.unwrap();
        assert_eq!(result.response_code, ResponseCode::NoError);
    }
}
