use std::{net::SocketAddr, sync::Arc};

use anyhow::Context;
use mihomo_subscription::{
    app::{build_router, AppState},
    auth::{AdminAuth, SessionStore, SESSION_IDLE},
    db,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mihomo_subscription=info,tower_http=info".into()),
        )
        .init();

    // Admin credentials are required; refuse to start without them.
    let admin_username = require_env("ADMIN_USERNAME")?;
    let admin_password = require_env("ADMIN_PASSWORD")?;

    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "/data".to_string());
    std::fs::create_dir_all(&data_dir)?;
    let db_path = format!("{data_dir}/mihomo-subscription.db");

    let pool = db::connect(&db_path).await?;
    let public_path_prefix =
        db::seed_public_path_prefix(&pool, std::env::var("PUBLIC_PATH_PREFIX").ok()).await?;
    tracing::info!("Database ready at {db_path}");

    // Use Secure cookies when the public origin is HTTPS.
    let secure_cookies = std::env::var("PUBLIC_BASE_URL")
        .map(|u| u.starts_with("https://"))
        .unwrap_or(false);
    let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "web/dist".to_string());

    let state = Arc::new(AppState {
        db: pool,
        public_path_prefix,
        admin: AdminAuth::new(&admin_username, &admin_password),
        sessions: SessionStore::new(SESSION_IDLE),
        secure_cookies,
        web_dir,
    });

    let app = build_router(state);

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

fn require_env(key: &str) -> anyhow::Result<String> {
    let value = std::env::var(key)
        .with_context(|| format!("{key} must be set (configured via the 1Panel install form)"))?;
    if value.is_empty() {
        anyhow::bail!("{key} must not be empty");
    }
    Ok(value)
}
