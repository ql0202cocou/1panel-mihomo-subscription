//! Shared test helpers.

#![allow(dead_code)] // each integration test binary uses a different subset

use std::sync::{Arc, RwLock};
use std::time::Duration;

use mihomo_subscription::app::AppState;
use mihomo_subscription::auth::{AdminAuth, SessionStore, SESSION_IDLE};
use mihomo_subscription::db;
use mihomo_subscription::fetch::{HttpFetcher, SubscriptionFetcher};
use mihomo_subscription::single_flight::SingleFlight;
use sqlx::SqlitePool;

/// A unique temp database path per test run; files removed on drop.
pub struct TempDb {
    pub path: String,
}

impl TempDb {
    pub fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!("mihomo-test-{}.db", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();
        Self { path }
    }

    pub async fn pool(&self) -> SqlitePool {
        db::connect(&self.path).await.unwrap()
    }
}

impl Default for TempDb {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path));
        }
    }
}

/// Build an `AppState` backed by a fresh temp database, with fixed admin
/// credentials `admin` / `s3cret` and a known base URL and path prefix. Uses a
/// real (never-called in CRUD tests) HTTP fetcher.
pub async fn test_state(temp: &TempDb) -> Arc<AppState> {
    let fetcher = Arc::new(HttpFetcher {
        timeout: Duration::from_secs(5),
        max_bytes: 1024 * 1024,
    });
    test_state_with_fetcher(temp, fetcher).await
}

/// Build an `AppState` with a caller-supplied fetcher (for generate/public
/// tests that must avoid real network access).
pub async fn test_state_with_fetcher(
    temp: &TempDb,
    fetcher: Arc<dyn SubscriptionFetcher>,
) -> Arc<AppState> {
    let pool = temp.pool().await;
    Arc::new(AppState {
        db: pool,
        public_base_url: "https://sub.example.com".into(),
        public_path_prefix: Arc::new(RwLock::new("testprefix".into())),
        admin: AdminAuth::new("admin", "s3cret"),
        sessions: SessionStore::new(SESSION_IDLE),
        secure_cookies: false,
        web_dir: "web/dist".into(),
        fetcher,
        cache_ttl: Duration::from_secs(15 * 60),
        single_flight: SingleFlight::new(),
    })
}
