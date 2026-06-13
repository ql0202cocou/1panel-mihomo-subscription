//! In-memory token-bucket rate limiting and the login/download middleware.
//!
//! A token bucket (rather than a fixed window) smooths traffic and avoids the
//! ~2x burst a fixed window allows at its boundaries. Each key holds `max`
//! tokens that refill continuously at `max / window` per second; a request is
//! allowed when at least one whole token is available.
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

/// A token-bucket rate limiter keyed by an arbitrary string.
pub struct RateLimiter {
    inner: Mutex<HashMap<String, Bucket>>,
    /// Bucket capacity (the burst size and steady-state max per window).
    capacity: f64,
    /// Tokens added per second.
    refill_per_sec: f64,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    /// `max` requests per `window` (also the burst capacity).
    pub fn new(max: u32, window: Duration) -> Self {
        let capacity = max as f64;
        let secs = window.as_secs_f64().max(f64::MIN_POSITIVE);
        Self {
            inner: Mutex::new(HashMap::new()),
            capacity,
            refill_per_sec: capacity / secs,
        }
    }

    /// Record a hit for `key`; return `true` if a token was available.
    pub fn check(&self, key: &str) -> bool {
        let mut map = self.inner.lock().unwrap();
        let now = Instant::now();

        // Opportunistic cleanup to bound memory under many distinct keys. A
        // fully refilled bucket is indistinguishable from a fresh one, so any
        // bucket idle for a full window can be dropped without affecting limits.
        if map.len() > 10_000 {
            let window_secs = self.capacity / self.refill_per_sec;
            map.retain(|_, b| now.duration_since(b.last).as_secs_f64() < window_secs);
        }

        let bucket = map.entry(key.to_string()).or_insert(Bucket {
            tokens: self.capacity,
            last: now,
        });
        // Refill for the elapsed time, capped at capacity.
        let elapsed = now.duration_since(bucket.last).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        bucket.last = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
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

/// Limit public subscription requests per client IP, independent of the token
/// in the path. Keying by IP only (not IP+path) means a client guessing many
/// distinct tokens shares one budget, so the limiter actually throttles token
/// enumeration / scanning — and runs before the handler, so 404s count too.
pub async fn download(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let key = format!("dl:{}", client_ip(&state, &req));
    if !state.download_limiter.check(&key) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_burst_up_to_capacity_then_denies() {
        let rl = RateLimiter::new(3, Duration::from_secs(60));
        assert!(rl.check("k"));
        assert!(rl.check("k"));
        assert!(rl.check("k"));
        assert!(!rl.check("k"), "4th hit over the burst capacity is denied");
        // A different key has its own bucket.
        assert!(rl.check("other"));
    }

    #[test]
    fn tokens_refill_over_time() {
        let rl = RateLimiter::new(1, Duration::from_millis(20));
        assert!(rl.check("k"));
        assert!(!rl.check("k"));
        std::thread::sleep(Duration::from_millis(30));
        assert!(rl.check("k"), "token refilled after the window elapsed");
    }

    #[test]
    fn partial_refill_grants_partial_budget() {
        // 10 tokens / 100ms = 1 token per 10ms. Drain, wait ~50ms, expect a
        // partial (not full) budget back — the hallmark of a token bucket vs a
        // fixed window, which would hand back the whole budget at the boundary.
        let rl = RateLimiter::new(10, Duration::from_millis(100));
        for _ in 0..10 {
            assert!(rl.check("k"));
        }
        assert!(!rl.check("k"), "bucket drained");

        std::thread::sleep(Duration::from_millis(50));
        let granted = (0..10).filter(|_| rl.check("k")).count();
        assert!(
            (3..=7).contains(&granted),
            "expected a partial refill (~5), got {granted}"
        );
    }
}
