//! 按 key 的 single-flight 锁。
//!
//! 用于让针对同一 profile 的并发公开请求合并为一次机场刷新,而非踩踏上游
//! (见 `docs/security-design.md`)。

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

    /// 取 `key` 对应的异步锁,首次使用时创建。在整个刷新期间持有返回的 guard,
    /// 使每个 key 同一时刻只有一个刷新在跑。
    pub fn lock_for(&self, key: &str) -> Arc<AsyncMutex<()>> {
        let mut map = self.locks.lock().unwrap();
        map.entry(key.to_string()).or_default().clone()
    }
}
