use std::{net::SocketAddr, sync::Arc};

use axum::{response::IntoResponse, routing::get, Json, Router};
use mihomo_subscription::db;
use sqlx::SqlitePool;
use tower_http::trace::TraceLayer;

// ─── App State ────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)] // wired up by the auth/profiles tasks
    pub db: SqlitePool,
    #[allow(dead_code)]
    pub public_path_prefix: String,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
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
    let db_path = format!("{data_dir}/mihomo-subscription.db");

    let pool = db::connect(&db_path).await?;
    let public_path_prefix =
        db::seed_public_path_prefix(&pool, std::env::var("PUBLIC_PATH_PREFIX").ok()).await?;
    tracing::info!("Database ready at {db_path}");

    let state = Arc::new(AppState {
        db: pool,
        public_path_prefix,
    });

    let app = Router::new()
        .route("/health", get(health))
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
