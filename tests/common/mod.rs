//! Shared test helpers.

use mihomo_subscription::db;
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
