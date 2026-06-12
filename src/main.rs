use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use uuid::Uuid;

// ─── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub last_updated: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubscription {
    pub name: String,
    pub url: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubscription {
    pub name: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
}

// ─── App State ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let subs = sqlx::query_as::<_, Subscription>("SELECT * FROM subscriptions ORDER BY created_at DESC")
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    Ok(Json(subs))
}

async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSubscription>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let enabled = payload.enabled.unwrap_or(true);

    sqlx::query(
        "INSERT INTO subscriptions (id, name, url, enabled, created_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&payload.name)
    .bind(&payload.url)
    .bind(enabled)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    let sub = Subscription {
        id,
        name: payload.name,
        url: payload.url,
        enabled,
        last_updated: None,
        created_at: now,
    };

    Ok((StatusCode::CREATED, Json(sub)))
}

async fn get_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let sub = sqlx::query_as::<_, Subscription>("SELECT * FROM subscriptions WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Subscription not found"})),
        ))?;

    Ok(Json(sub))
}

async fn update_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSubscription>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let existing = sqlx::query_as::<_, Subscription>("SELECT * FROM subscriptions WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Subscription not found"})),
        ))?;

    let name = payload.name.unwrap_or(existing.name);
    let url = payload.url.unwrap_or(existing.url);
    let enabled = payload.enabled.unwrap_or(existing.enabled);

    sqlx::query("UPDATE subscriptions SET name = ?, url = ?, enabled = ? WHERE id = ?")
        .bind(&name)
        .bind(&url)
        .bind(enabled)
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    let updated = Subscription {
        id,
        name,
        url,
        enabled,
        last_updated: existing.last_updated,
        created_at: existing.created_at,
    };

    Ok(Json(updated))
}

async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let result = sqlx::query("DELETE FROM subscriptions WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Subscription not found"})),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

/// 获取合并后的 Mihomo 配置（合并所有已启用的订阅）
async fn get_merged_config(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let subs = sqlx::query_as::<_, Subscription>(
        "SELECT * FROM subscriptions WHERE enabled = 1 ORDER BY created_at ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
    })?;

    // 返回已启用的订阅列表供 Mihomo 外部提供商使用
    let payload = serde_json::json!({
        "enabled_subscriptions": subs.iter().map(|s| &s.url).collect::<Vec<_>>(),
        "count": subs.len()
    });

    Ok(Json(payload))
}

// ─── Database Init ────────────────────────────────────────────────────────────

async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS subscriptions (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            url         TEXT NOT NULL,
            enabled     BOOLEAN NOT NULL DEFAULT 1,
            last_updated TEXT,
            created_at  TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// ─── Entry Point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mihomo_subscription=info,tower_http=info".into()),
        )
        .init();

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "/data".to_string());
    std::fs::create_dir_all(&data_dir)?;
    let db_url = format!("sqlite:{}/mihomo-subscription.db", data_dir);

    let pool = SqlitePool::connect(&db_url).await?;
    init_db(&pool).await?;

    let state = Arc::new(AppState { db: pool });

    let api_routes = Router::new()
        .route("/subscriptions", get(list_subscriptions).post(create_subscription))
        .route(
            "/subscriptions/:id",
            get(get_subscription)
                .put(update_subscription)
                .delete(delete_subscription),
        )
        .route("/merged", get(get_merged_config));

    let app = Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_routes)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    Ok(())
}
