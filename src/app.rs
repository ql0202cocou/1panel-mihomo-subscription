//! Application state and HTTP router assembly, shared by the binary and the
//! integration tests.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::{
    extract::DefaultBodyLimit,
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use sqlx::SqlitePool;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth::{self, AdminAuth, SessionStore};
use crate::fetch::SubscriptionFetcher;
use crate::rate_limit::{self, RateLimiter};
use crate::single_flight::SingleFlight;
use crate::{generate, profiles, settings};

/// Maximum management request body size (see `docs/api-design.md`).
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// Externally reachable origin used to assemble hosted links.
    pub public_base_url: String,
    /// The global public path prefix. Held behind a lock because
    /// reset-public-path updates it at runtime (see `docs/security-design.md`).
    pub public_path_prefix: Arc<RwLock<String>>,
    pub admin: AdminAuth,
    pub sessions: SessionStore,
    /// Set the `Secure` cookie attribute (true when served behind HTTPS).
    pub secure_cookies: bool,
    /// Directory of the built SPA assets to serve.
    pub web_dir: String,
    /// Provider fetcher (real SSRF-protected client in production).
    pub fetcher: Arc<dyn SubscriptionFetcher>,
    /// Generated-cache TTL.
    pub cache_ttl: Duration,
    /// Per-profile refresh coalescing.
    pub single_flight: SingleFlight,
    /// Reverse-proxy hops to trust when deriving the client IP.
    pub trusted_proxy_hops: usize,
    /// Login attempt limiter (keyed by client IP).
    pub login_limiter: Arc<RateLimiter>,
    /// Public download limiter (keyed by client IP; throttles enumeration).
    pub download_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn current_prefix(&self) -> String {
        self.public_path_prefix.read().unwrap().clone()
    }

    pub fn set_prefix(&self, prefix: String) {
        *self.public_path_prefix.write().unwrap() = prefix;
    }

    /// Assemble a profile's permanent subscription URL.
    pub fn subscription_url(&self, token: &str) -> String {
        format!(
            "{}/{}/api/sub/{}",
            self.public_base_url.trim_end_matches('/'),
            self.current_prefix(),
            token
        )
    }
}

async fn health() -> impl IntoResponse {
    // Intentionally minimal: no version, to avoid unauthenticated disclosure.
    Json(serde_json::json!({"status": "ok"}))
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
        .route("/profiles", get(profiles::list).post(profiles::create))
        .route(
            "/profiles/:id",
            get(profiles::get)
                .put(profiles::update)
                .delete(profiles::delete),
        )
        .route("/profiles/:id/reset-token", post(profiles::reset_token))
        .route("/profiles/:id/generate", post(generate::generate))
        .route("/profiles/:id/preview", get(generate::preview))
        .route(
            "/profiles/:id/provider-rules",
            get(generate::provider_rules),
        )
        .route("/profiles/:id/rules", put(profiles::put_rules))
        .route("/profiles/:id/proxies", get(profiles::list_proxies))
        .route("/profiles/:id/node-order", put(profiles::set_node_order))
        .route("/profiles/:id/group-order", put(profiles::set_group_order))
        .route(
            "/profiles/:id/nodes",
            get(profiles::list_nodes).post(profiles::create_node),
        )
        .route(
            "/profiles/:id/nodes/:node_id",
            put(profiles::update_node).delete(profiles::delete_node),
        )
        .route(
            "/profiles/:id/groups",
            get(profiles::list_groups).post(profiles::create_group),
        )
        .route(
            "/profiles/:id/import-provider-groups",
            post(profiles::import_provider_groups),
        )
        .route(
            "/profiles/:id/groups/:group_id",
            put(profiles::update_group).delete(profiles::delete_group),
        )
        .route("/settings", get(settings::get))
        .route(
            "/settings/reset-public-path",
            post(settings::reset_public_path),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_session,
        ));

    let login_route = post(auth::login).layer(middleware::from_fn_with_state(
        state.clone(),
        rate_limit::login,
    ));

    let api = Router::new()
        .route("/auth/login", login_route)
        .merge(protected)
        .layer(middleware::from_fn(auth::check_origin))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));

    let spa = ServeDir::new(&state.web_dir)
        .fallback(ServeFile::new(format!("{}/index.html", state.web_dir)));

    Router::new()
        .route("/health", get(health))
        .nest("/api", api)
        // Public subscription download: no auth, path prefix + token, but
        // rate-limited by client IP + path.
        .route(
            "/:public_path_prefix/api/sub/:token",
            get(generate::public_sub).layer(middleware::from_fn_with_state(
                state.clone(),
                rate_limit::download,
            )),
        )
        .fallback_service(spa)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
