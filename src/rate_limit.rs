//! In-memory fixed-window rate limiting and the login/download middleware.
//!
//! In-memory limits are acceptable for the single-instance self-hosted MVP
//! (see `docs/security-design.md`); a shared store would be needed only if the
//! app later runs multi-instance.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::app::AppState;
use crate::net;

/// A fixed-window counter keyed by an arbitrary string.
pub struct RateLimiter {
    inner: Mutex<HashMap<String, Window>>,
    max: u32,
    window: Duration,
}

struct Window {
    start: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn new(max: u32, window: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            max,
            window,
        }
    }

    /// Record a hit for `key`; return `true` if it is within the limit.
    pub fn check(&self, key: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();

        // Opportunistic cleanup to bound memory under many distinct keys.
        if map.len() > 10_000 {
            map.retain(|_, w| now.duration_since(w.start) < self.window);
        }

        match map.get_mut(key) {
            Some(w) if now.duration_since(w.start) < self.window => {
                if w.count >= self.max {
                    return false;
                }
                w.count += 1;
                true
            }
            _ => {
                map.insert(
                    key.to_string(),
                    Window {
                        start: now,
                        count: 1,
                    },
                );
                true
            }
        }
    }
}

/// Derive the client IP string for `req` using the app's trusted-proxy config.
fn client_ip(state: &AppState, req: &Request) -> String {
    let xff = req
        .headers()
        .get(header::HeaderName::from_static("x-forwarded-for"))
        .and_then(|v| v.to_str().ok());
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip());
    net::client_ip_string(xff, peer, state.trusted_proxy_hops)
}

/// Limit login attempts by client IP (brute-force control).
pub async fn login(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let key = format!("login:{}", client_ip(&state, &req));
    if !state.login_limiter.check(&key) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(req).await
}

/// Limit public subscription downloads by client IP + request path (which
/// includes the path prefix and token).
pub async fn download(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let key = format!("dl:{}:{}", client_ip(&state, &req), req.uri().path());
    if !state.download_limiter.check(&key) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_up_to_max_then_denies() {
        let rl = RateLimiter::new(3, Duration::from_secs(60));
        assert!(rl.check("k"));
        assert!(rl.check("k"));
        assert!(rl.check("k"));
        assert!(!rl.check("k"), "4th hit over the limit is denied");
        // A different key has its own budget.
        assert!(rl.check("other"));
    }

    #[test]
    fn window_resets_after_expiry() {
        let rl = RateLimiter::new(1, Duration::from_millis(20));
        assert!(rl.check("k"));
        assert!(!rl.check("k"));
        std::thread::sleep(Duration::from_millis(30));
        assert!(rl.check("k"), "new window after expiry");
    }
}
