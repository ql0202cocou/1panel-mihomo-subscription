//! Admin authentication: constant-time credential check, in-memory sessions,
//! session-cookie handlers, and the auth / Origin middleware.
//!
//! Behavior follows `docs/security-design.md` (Admin Authentication, CORS and
//! CSRF) and `docs/api-design.md` (Authentication).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::app::AppState;

const SESSION_COOKIE: &str = "session";
/// Idle session lifetime, per `docs/security-design.md` (default 7 days).
pub const SESSION_IDLE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

// ─── Credentials ──────────────────────────────────────────────────────────────

/// Holds the admin username and a fixed-size digest of the credential pair.
/// Verification hashes the submitted pair and compares digests in constant
/// time, so neither the result nor the input length leaks via timing.
#[derive(Clone)]
pub struct AdminAuth {
    username: String,
    digest: [u8; 32],
}

impl AdminAuth {
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: username.to_string(),
            digest: credential_digest(username, password),
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn verify(&self, username: &str, password: &str) -> bool {
        let candidate = credential_digest(username, password);
        candidate.ct_eq(&self.digest).into()
    }
}

fn credential_digest(username: &str, password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update([0u8]); // separator so (user, pass) pairs can't be confused
    hasher.update(password.as_bytes());
    hasher.finalize().into()
}

// ─── Session store ────────────────────────────────────────────────────────────

/// In-memory session store keyed by a random session ID. Sessions are dropped
/// on restart (acceptable for a single-instance self-hosted app) and expire
/// after a bounded idle time, refreshed on each authenticated request.
#[derive(Clone)]
pub struct SessionStore {
    inner: Arc<Mutex<HashMap<String, Instant>>>,
    idle: Duration,
}

impl SessionStore {
    pub fn new(idle: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            idle,
        }
    }

    /// Create a new session and return its ID (256 bits of CSPRNG entropy).
    pub fn create(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let mut map = self.inner.lock().unwrap();
        // Sweep expired sessions on creation. Without this, idle/expired entries
        // linger until the same ID is validated again (which never happens for
        // an abandoned session), so the map could only grow. Creation is the
        // sole growth point, so sweeping here bounds it.
        let idle = self.idle;
        map.retain(|_, last_seen| last_seen.elapsed() <= idle);
        map.insert(id.clone(), Instant::now());
        id
    }

    /// Return true if the session exists and is within the idle window,
    /// refreshing its last-seen time. Expired sessions are removed.
    pub fn validate(&self, id: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        match map.get(id) {
            Some(last_seen) if last_seen.elapsed() <= self.idle => {
                map.insert(id.to_string(), Instant::now());
                true
            }
            Some(_) => {
                map.remove(id);
                false
            }
            None => false,
        }
    }

    pub fn remove(&self, id: &str) {
        self.inner.lock().unwrap().remove(id);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct SessionResponse {
    username: String,
}

pub async fn login(State(state): State<Arc<AppState>>, Json(body): Json<LoginRequest>) -> Response {
    // NOTE: login-failure rate limiting is added in the rate-limit task (#8).
    if state.admin.verify(&body.username, &body.password) {
        let id = state.sessions.create();
        let cookie = build_cookie(&id, state.secure_cookies, SESSION_IDLE.as_secs());
        (StatusCode::NO_CONTENT, [(header::SET_COOKIE, cookie)]).into_response()
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

pub async fn logout(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if let Some(id) = session_cookie(&req) {
        state.sessions.remove(&id);
    }
    let cleared = build_cookie("", state.secure_cookies, 0);
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, cleared)]).into_response()
}

pub async fn session(State(state): State<Arc<AppState>>) -> Response {
    // Reaches here only past `require_session`, so a session is valid.
    Json(SessionResponse {
        username: state.admin.username().to_string(),
    })
    .into_response()
}

// ─── Middleware ───────────────────────────────────────────────────────────────

/// Require a valid session cookie; otherwise `401`.
pub async fn require_session(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    match session_cookie(&req) {
        Some(id) if state.sessions.validate(&id) => next.run(req).await,
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// Defense in depth against CSRF: when a state-changing request carries an
/// `Origin` header, it must match the request `Host`. Same-origin SPA requests
/// satisfy this; cross-site form posts do not. Requests without `Origin` are
/// left to the `SameSite=Lax` cookie attribute.
pub async fn check_origin(req: Request, next: Next) -> Response {
    let state_changing = matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    );
    if state_changing {
        if let Some(origin) = req.headers().get(header::ORIGIN) {
            let host = req.headers().get(header::HOST);
            if !origin_matches_host(origin.to_str().ok(), host.and_then(|h| h.to_str().ok())) {
                return StatusCode::FORBIDDEN.into_response();
            }
        }
    }
    next.run(req).await
}

fn origin_matches_host(origin: Option<&str>, host: Option<&str>) -> bool {
    match (origin, host) {
        (Some(origin), Some(host)) => origin.split_once("://").map(|(_, a)| a) == Some(host),
        _ => false,
    }
}

// ─── Cookie helpers ───────────────────────────────────────────────────────────

fn build_cookie(value: &str, secure: bool, max_age: u64) -> String {
    let mut cookie =
        format!("{SESSION_COOKIE}={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn session_cookie(req: &Request) -> Option<String> {
    let header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    header.split(';').find_map(|pair| {
        let (name, value) = pair.trim().split_once('=')?;
        (name == SESSION_COOKIE).then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_check_is_pair_bound() {
        let admin = AdminAuth::new("admin", "s3cret");
        assert!(admin.verify("admin", "s3cret"));
        assert!(!admin.verify("admin", "wrong"));
        assert!(!admin.verify("other", "s3cret"));
        // The separator prevents (user, pass) concatenation confusion.
        assert!(!admin.verify("admins3cret", ""));
        assert!(!admin.verify("admin\0s3cret", ""));
    }

    #[test]
    fn create_sweeps_expired_sessions() {
        let store = SessionStore::new(Duration::from_millis(20));
        let stale = store.create();
        assert_eq!(store.len(), 1);

        std::thread::sleep(Duration::from_millis(30));
        // Creating a fresh session sweeps the now-expired one.
        let fresh = store.create();
        assert_eq!(store.len(), 1, "expired session swept on create");
        assert!(store.validate(&fresh));
        assert!(!store.validate(&stale));
    }

    #[test]
    fn origin_must_match_host() {
        assert!(origin_matches_host(
            Some("https://sub.example.com"),
            Some("sub.example.com")
        ));
        assert!(!origin_matches_host(
            Some("https://evil.example.org"),
            Some("sub.example.com")
        ));
        assert!(!origin_matches_host(None, Some("sub.example.com")));
        assert!(!origin_matches_host(Some("https://sub.example.com"), None));
    }
}
