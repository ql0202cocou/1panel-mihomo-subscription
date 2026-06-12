//! Database setup: connection pool, migrations, and app-settings seeding.
//!
//! Per `docs/data-model.md`, `foreign_keys` and `busy_timeout` are
//! per-connection pragmas. They are configured on `SqliteConnectOptions`, which
//! SQLx issues on *every* physical connection it opens — the idiomatic
//! equivalent of an after-connect hook — so `ON DELETE CASCADE` cannot be
//! silently disabled on some pooled connections. `journal_mode = WAL` is set
//! once and persists with the database file.

use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use uuid::Uuid;

/// Build the canonical per-connection options for a database file path.
pub fn connect_options(path: &str) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
}

/// Open a pool from the given options and run pending migrations.
pub async fn connect_with(options: SqliteConnectOptions) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Open a pool for a database file at `path` and run pending migrations.
pub async fn connect(path: &str) -> Result<SqlitePool> {
    connect_with(connect_options(path)).await
}

/// Ensure the single `app_settings` row exists and return the public path
/// prefix. On first startup the prefix is seeded from `env_seed` when present
/// and non-empty, otherwise a random URL-safe value is generated. Subsequent
/// startups return the persisted value (which may have been reset at runtime).
pub async fn seed_public_path_prefix(
    pool: &SqlitePool,
    env_seed: Option<String>,
) -> Result<String> {
    if let Some((prefix,)) =
        sqlx::query_as::<_, (String,)>("SELECT public_path_prefix FROM app_settings WHERE id = 1")
            .fetch_optional(pool)
            .await?
    {
        return Ok(prefix);
    }

    let prefix = env_seed
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(random_path_prefix);
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO app_settings (id, public_path_prefix, updated_at) VALUES (1, ?, ?)")
        .bind(&prefix)
        .bind(&now)
        .execute(pool)
        .await?;
    Ok(prefix)
}

/// A random URL-safe path segment (22 chars), within the 16-24 char range
/// recommended in `docs/security-design.md`.
fn random_path_prefix() -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Uuid::new_v4().into_bytes())
}
