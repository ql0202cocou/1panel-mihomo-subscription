//! 管理员认证:恒定时间凭据校验、内存会话、会话 cookie 处理器,以及 auth / Origin 中间件。
//!
//! 行为遵循 `docs/security-design.md`(管理员认证、CORS 与 CSRF)与 `docs/api-design.md`(认证)。

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
/// 会话空闲生命周期,见 `docs/security-design.md`(默认 7 天)。
pub const SESSION_IDLE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

// ─── 凭据 ──────────────────────────────────────────────────────────────────────

/// 持有管理员用户名与凭据对的定长摘要。校验时对提交的凭据对做哈希,并恒定时间比较摘要,
/// 故结果与输入长度都不会经由时序泄露。
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
    hasher.update([0u8]); // 分隔符,使 (user, pass) 对不会混淆
    hasher.update(password.as_bytes());
    hasher.finalize().into()
}

// ─── 会话存储 ──────────────────────────────────────────────────────────────────

/// 以随机会话 ID 为 key 的内存会话存储。会话在重启时丢弃(单实例自托管应用可接受),并在有限的
/// 空闲时间后过期,每次已认证请求都会刷新。
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

    /// 创建新会话并返回其 ID(256 位 CSPRNG 熵)。
    pub fn create(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        let mut map = self.inner.lock().unwrap();
        // 创建时清扫过期会话。否则空闲/过期条目会一直滞留,直到同一 ID 再次被校验(对被遗弃的
        // 会话永不会发生),map 只增不减。创建是唯一的增长点,故在此清扫即可限制它。
        let idle = self.idle;
        map.retain(|_, last_seen| last_seen.elapsed() <= idle);
        map.insert(id.clone(), Instant::now());
        id
    }

    /// 会话存在且在空闲窗口内则返回 true,并刷新其 last-seen 时间。过期会话会被移除。
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

// ─── 处理器 ────────────────────────────────────────────────────────────────────

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
    // 注:登录失败限流由 rate-limit 任务(#8)添加。
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
    // 只有过了 `require_session` 才会到这里,故会话有效。
    Json(SessionResponse {
        username: state.admin.username().to_string(),
    })
    .into_response()
}

// ─── 中间件 ────────────────────────────────────────────────────────────────────

/// 要求有效的会话 cookie;否则 `401`。
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

/// 针对 CSRF 的纵深防御:状态变更请求带 `Origin` 头时,它必须匹配请求的 `Host`。同源 SPA 请求
/// 满足此条件;跨站表单提交不满足。无 `Origin` 的请求交由 `SameSite=Lax` cookie 属性兜底。
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

// ─── Cookie 辅助 ───────────────────────────────────────────────────────────────

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
