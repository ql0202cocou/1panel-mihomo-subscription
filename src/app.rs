//! Application state and HTTP router assembly, shared by the binary and the
//! integration tests.

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use sqlx::SqlitePool;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth::{self, AdminAuth, SessionStore};

/// Maximum management request body size (see `docs/api-design.md`).
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub public_path_prefix: String,
    pub admin: AdminAuth,
    pub sessions: SessionStore,
    /// Set the `Secure` cookie attribute (true when served behind HTTPS).
    pub secure_cookies: bool,
    /// Directory of the built SPA assets to serve.
    pub web_dir: String,
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

/// Build the full application router.
///
/// Layering: `/health` and the login endpoint are public; every other `/api`
/// route requires a valid session. The management API has no CORS layer (it is
/// same-origin) but does enforce an `Origin` check on state-changing requests
/// and a request body size limit. Unmatched paths fall through to the SPA with
/// an `index.html` fallback.
pub fn build_router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/auth/session", get(auth::session))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    let api = Router::new()
        .route("/auth/login", post(auth::login))
        .merge(protected)
        .layer(middleware::from_fn(auth::check_origin))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));

    let spa = ServeDir::new(&state.web_dir)
        .fallback(ServeFile::new(format!("{}/index.html", state.web_dir)));

    Router::new()
        .route("/health", get(health))
        .nest("/api", api)
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
