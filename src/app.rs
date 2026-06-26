//! 应用状态与 HTTP 路由组装,供二进制与集成测试共用。

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
use crate::{generate, global_nodes, profiles, settings};

/// 管理请求体大小上限(见 `docs/api-design.md`)。
const MAX_BODY_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    /// 外部可达的源,用于拼装托管链接。
    pub public_base_url: String,
    /// 全局公共路径前缀。用锁持有,因为 reset-public-path 会在运行时更新它
    /// (见 `docs/security-design.md`)。
    pub public_path_prefix: Arc<RwLock<String>>,
    pub admin: AdminAuth,
    pub sessions: SessionStore,
    /// 是否设置 `Secure` cookie 属性(HTTPS 提供时为 true)。
    pub secure_cookies: bool,
    /// 要提供的已构建 SPA 资产目录。
    pub web_dir: String,
    /// 机场获取器(生产中是真实的 SSRF 保护客户端)。
    pub fetcher: Arc<dyn SubscriptionFetcher>,
    /// 生成缓存的 TTL。
    pub cache_ttl: Duration,
    /// per-profile 的刷新合并。
    pub single_flight: SingleFlight,
    /// 推导客户端 IP 时信任的反向代理跳数。
    pub trusted_proxy_hops: usize,
    /// 登录尝试限流器(按客户端 IP)。
    pub login_limiter: Arc<RateLimiter>,
    /// 公开下载限流器(按客户端 IP;抑制枚举)。
    pub download_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn current_prefix(&self) -> String {
        self.public_path_prefix.read().unwrap().clone()
    }

    pub fn set_prefix(&self, prefix: String) {
        *self.public_path_prefix.write().unwrap() = prefix;
    }

    /// 拼装一条 profile 的永久订阅 URL。
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
    // 有意保持最小:不含版本,避免未认证泄露。
    Json(serde_json::json!({"status": "ok"}))
}

/// 构建完整的应用路由。
///
/// 分层:`/health` 与登录端点公开;其余每条 `/api` 路由都需要有效会话。管理 API 无 CORS 层
/// (它同源),但对状态变更请求强制 `Origin` 校验,并限制请求体大小。未匹配的路径落到 SPA,
/// 以 `index.html` 兜底。
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
        .route(
            "/profiles/:id/node-section-order",
            put(profiles::set_node_section_order),
        )
        .route("/profiles/:id/group-order", put(profiles::set_group_order))
        // 全局自定义节点(跨订阅池):增删改 + 排序。
        .route(
            "/global-nodes",
            get(global_nodes::list).post(global_nodes::create),
        )
        .route("/global-nodes/order", put(global_nodes::set_order))
        .route(
            "/global-nodes/:id",
            put(global_nodes::update).delete(global_nodes::delete),
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
        // 公开订阅下载:无鉴权,路径前缀 + token,但按客户端 IP + 路径限流。
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
