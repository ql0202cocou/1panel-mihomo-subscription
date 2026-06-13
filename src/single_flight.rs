//! Per-key single-flight locks.
//!
//! Used so concurrent public requests for the same profile coalesce into one
//! provider refresh instead of stampeding the upstream (see
//! `docs/security-design.md`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::Mutex as AsyncMutex;

#[derive(Clone, Default)]
pub struct SingleFlight {
    locks: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
}

impl SingleFlight {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the async lock for `key`, creating it on first use. Hold the
    /// returned guard across the refresh so only one runs per key at a time.
    pub fn lock_for(&self, key: &str) -> Arc<AsyncMutex<()>> {
        let mut map = self.locks.lock().unwrap();
        map.entry(key.to_string()).or_default().clone()
    }
}
