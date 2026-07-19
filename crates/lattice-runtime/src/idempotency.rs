//! Bounded idempotency cache for mutation retries within a workspace session.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Default number of recent idempotency keys retained per session.
pub const DEFAULT_IDEMPOTENCY_CAPACITY: usize = 256;

/// Cached outcome of a successful mutation keyed by caller idempotency key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotentOutcome {
    /// Resulting content-hash revision after the mutation.
    pub revision: String,
}

/// FIFO-bounded map from idempotency key → outcome.
#[derive(Debug)]
pub struct IdempotencyCache {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    capacity: usize,
    order: VecDeque<String>,
    entries: HashMap<String, IdempotentOutcome>,
}

impl IdempotencyCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                capacity: capacity.max(1),
                order: VecDeque::new(),
                entries: HashMap::new(),
            }),
        }
    }

    pub fn get(&self, key: &str) -> Option<IdempotentOutcome> {
        let guard = self.inner.lock().expect("idempotency cache poisoned");
        guard.entries.get(key).cloned()
    }

    pub fn insert(&self, key: impl Into<String>, outcome: IdempotentOutcome) {
        let key = key.into();
        let mut guard = self.inner.lock().expect("idempotency cache poisoned");
        if guard.entries.contains_key(&key) {
            guard.entries.insert(key, outcome);
            return;
        }
        while guard.order.len() >= guard.capacity {
            if let Some(evicted) = guard.order.pop_front() {
                guard.entries.remove(&evicted);
            } else {
                break;
            }
        }
        guard.order.push_back(key.clone());
        guard.entries.insert(key, outcome);
    }

    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("idempotency cache poisoned")
            .entries
            .len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for IdempotencyCache {
    fn default() -> Self {
        Self::new(DEFAULT_IDEMPOTENCY_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_when_over_capacity() {
        let cache = IdempotencyCache::new(2);
        cache.insert(
            "a",
            IdempotentOutcome {
                revision: "r1".into(),
            },
        );
        cache.insert(
            "b",
            IdempotentOutcome {
                revision: "r2".into(),
            },
        );
        cache.insert(
            "c",
            IdempotentOutcome {
                revision: "r3".into(),
            },
        );
        assert!(cache.get("a").is_none());
        assert_eq!(cache.get("b").unwrap().revision, "r2");
        assert_eq!(cache.get("c").unwrap().revision, "r3");
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn get_returns_cached_outcome() {
        let cache = IdempotencyCache::new(4);
        assert!(cache.get("k").is_none());
        cache.insert(
            "k",
            IdempotentOutcome {
                revision: "sha256:abc".into(),
            },
        );
        assert_eq!(cache.get("k").unwrap().revision, "sha256:abc");
    }
}
