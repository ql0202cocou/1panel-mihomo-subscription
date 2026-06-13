use std::{
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Context;
use mihomo_subscription::{
    app::{build_router, AppState},
    auth::{AdminAuth, SessionStore, SESSION_IDLE},
    db,
    fetch::HttpFetcher,
    rate_limit::RateLimiter,
    single_flight::SingleFlight,
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

    let public_base_url = std::env::var("PUBLIC_BASE_URL").unwrap_or_default();
    // Set the Secure cookie attribute. `SECURE_COOKIES` is an explicit override;
    // when unset we infer it from an HTTPS public origin. Behind a TLS-terminating
    // reverse proxy the app speaks plain HTTP, so without this override a missing
    // or http `PUBLIC_BASE_URL` would silently issue session cookies without
    // `Secure`, exposing them to plaintext transmission.
    let secure_cookies = env_bool("SECURE_COOKIES", public_base_url.starts_with("https://"));
    if !secure_cookies {
        tracing::warn!(
            "session cookies will be issued WITHOUT the Secure attribute; set \
             SECURE_COOKIES=true (or an https:// PUBLIC_BASE_URL) when serving over HTTPS"
        );
    }
    let web_dir = std::env::var("WEB_DIR").unwrap_or_else(|_| "web/dist".to_string());

    let fetch_timeout = Duration::from_secs(env_u64("FETCH_TIMEOUT_SECONDS", 15));
    let max_bytes = env_u64("MAX_SUBSCRIPTION_SIZE_MB", 8) as usize * 1024 * 1024;
    let cache_ttl = Duration::from_secs(env_u64("CACHE_TTL_MINUTES", 15) * 60);

    let state = Arc::new(AppState {
        db: pool,
        public_base_url,
        public_path_prefix: Arc::new(RwLock::new(public_path_prefix)),
        admin: AdminAuth::new(&admin_username, &admin_password),
        sessions: SessionStore::new(SESSION_IDLE),
        secure_cookies,
        web_dir,
        fetcher: Arc::new(HttpFetcher {
            timeout: fetch_timeout,
            max_bytes,
        }),
        cache_ttl,
        single_flight: SingleFlight::new(),
        trusted_proxy_hops: env_u64("TRUSTED_PROXY_HOPS", 1) as usize,
        login_limiter: Arc::new(RateLimiter::new(10, Duration::from_secs(60))),
        download_limiter: Arc::new(RateLimiter::new(120, Duration::from_secs(60))),
    });

    let app = build_router(state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Listening on http://{}", listener.local_addr()?);
    // Connect info exposes the TCP peer address for client-IP derivation.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Parse a boolean env var (`true`/`false`/`1`/`0`, case-insensitive); fall back
/// to `default` when unset or unrecognized.
fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => default,
        },
        Err(_) => default,
    }
}

fn require_env(key: &str) -> anyhow::Result<String> {
    let value = std::env::var(key)
        .with_context(|| format!("{key} must be set (configured via the 1Panel install form)"))?;
    if value.is_empty() {
        anyhow::bail!("{key} must not be empty");
    }
    Ok(value)
}
