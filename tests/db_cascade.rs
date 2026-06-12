//! Verifies that the per-connection `foreign_keys` pragma is actually applied
//! across the pool: deleting a profile must cascade to every child table. If
//! the pragma were set only once (not per connection), this would fail
//! intermittently or leave orphan rows.

use mihomo_subscription::db;
use sqlx::SqlitePool;

/// A unique temp database path per test run; removed on drop.
struct TempDb {
    path: String,
}

impl TempDb {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!("mihomo-test-{}.db", uuid::Uuid::new_v4()))
            .to_string_lossy()
            .into_owned();
        Self { path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path));
        }
    }
}

async fn seed_profile_with_children(pool: &SqlitePool, profile_id: &str) {
    let now = "2026-06-12T00:00:00Z";
    sqlx::query(
        "INSERT INTO profiles (id, name, source_type, source_url, token, created_at, updated_at)
         VALUES (?, 'p', 'clash', 'https://example.com/sub?token=x', 'tok', ?, ?)",
    )
    .bind(profile_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO rulesets (id, profile_id, content, updated_at) VALUES (?, ?, 'MATCH,DIRECT', ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(profile_id)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO custom_nodes (id, profile_id, name, node_type, content, created_at, updated_at)
         VALUES (?, ?, 'n', 'ss', '{}', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(profile_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO custom_groups (id, profile_id, name, group_type, members, created_at, updated_at)
         VALUES (?, ?, 'g', 'select', '[\"DIRECT\"]', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(profile_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO generated_cache (profile_id, content_hash, output_yaml, generated_at)
         VALUES (?, 'h', 'proxies: []', ?)",
    )
    .bind(profile_id)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
}

async fn count(pool: &SqlitePool, table: &str, profile_id: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE profile_id = ?");
    sqlx::query_scalar::<_, i64>(&sql)
        .bind(profile_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn delete_profile_cascades_to_all_children() {
    let temp = TempDb::new();
    let pool = db::connect(&temp.path).await.unwrap();
    let profile_id = uuid::Uuid::new_v4().to_string();

    seed_profile_with_children(&pool, &profile_id).await;

    for table in [
        "rulesets",
        "custom_nodes",
        "custom_groups",
        "generated_cache",
    ] {
        assert_eq!(count(&pool, table, &profile_id).await, 1, "seed {table}");
    }

    sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(&profile_id)
        .execute(&pool)
        .await
        .unwrap();

    for table in [
        "rulesets",
        "custom_nodes",
        "custom_groups",
        "generated_cache",
    ] {
        assert_eq!(
            count(&pool, table, &profile_id).await,
            0,
            "cascade should remove rows from {table}"
        );
    }
}

#[tokio::test]
async fn foreign_keys_pragma_is_on_for_pooled_connections() {
    let temp = TempDb::new();
    let pool = db::connect(&temp.path).await.unwrap();

    // Hit several pooled connections; each must report foreign_keys = 1.
    for _ in 0..10 {
        let on: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(on, 1, "foreign_keys must be ON for every pooled connection");
    }
}

#[tokio::test]
async fn seed_public_path_prefix_is_idempotent() {
    let temp = TempDb::new();
    let pool = db::connect(&temp.path).await.unwrap();

    let first = db::seed_public_path_prefix(&pool, Some("seeded-prefix".into()))
        .await
        .unwrap();
    assert_eq!(first, "seeded-prefix");

    // A later call must return the persisted value, ignoring a new seed.
    let second = db::seed_public_path_prefix(&pool, Some("different".into()))
        .await
        .unwrap();
    assert_eq!(second, "seeded-prefix");
}
