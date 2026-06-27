//! Per-profile 自包含规则库(③ 托管规则库)。
//!
//! 每个订阅持有自己的规则集定义;`RULE-SET,<name>,<policy>` 规则按名引用,生成时由
//! `src/generate.rs` 注入到输出 `rule-providers:`,`url` 指向 **按订阅 token 隔离** 的托管链接
//! `/<prefix>/api/sub/<token>/r/<name>/<behavior>.<format>`。订阅由此自包含:生成只读本表,不再
//! 依赖全局 `rule_sets`(② 仅作导入源)。提供 per-profile 增删改 + 从 ② 导入 + 公开托管端点。
//!
//! 两种来源与全局库一致(见 `src/rulelib.rs`):`manual` 托管录入 payload;`remote` 镜像远程规则集
//! (`cache=1` 懒刷新二次托管,支持二进制 `mrs`;`cache=0` 转换时直接注入上游 URL)。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::app::AppState;
use crate::error::{ApiError, ApiResult};
use crate::mask;
use crate::rulelib::{self, RuleSetBody};
use crate::util::now;

// ─── 行 / 响应 ──────────────────────────────────────────────────────────────────

/// 列表 / CRUD 用的元信息行(不含 BLOB `cached_body`)。
#[derive(FromRow)]
struct RuleSetRow {
    id: String,
    name: String,
    behavior: String,
    format: String,
    source: String,
    content: String,
    rule_count: i64,
    url: Option<String>,
    interval_hours: i64,
    cache: bool,
    last_fetch_status: Option<String>,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

const META_COLS: &str = "id, name, behavior, format, source, content, rule_count, url, \
     interval_hours, cache, last_fetch_status, enabled, created_at, updated_at";

#[derive(Serialize)]
pub struct ProfileRuleSetResponse {
    id: String,
    name: String,
    behavior: String,
    format: String,
    source: String,
    content: String,
    enabled: bool,
    /// 规则条数(manual=payload 行数;remote=最近成功镜像的行数,mrs 为 0)。
    count: i64,
    /// 按订阅 token 隔离的托管链接。
    url: String,
    /// 远程来源 URL(已脱敏);manual 为 null。
    remote_url_masked: Option<String>,
    interval_hours: i64,
    cache: bool,
    last_fetch_status: Option<String>,
    created_at: String,
    updated_at: String,
}

fn to_response(state: &AppState, token: &str, row: RuleSetRow) -> ProfileRuleSetResponse {
    ProfileRuleSetResponse {
        url: state.profile_rule_set_url(token, &row.name, &row.behavior, &row.format),
        remote_url_masked: row.url.as_deref().map(mask::mask_url),
        count: row.rule_count,
        id: row.id,
        name: row.name,
        behavior: row.behavior,
        format: row.format,
        source: row.source,
        content: row.content,
        enabled: row.enabled,
        interval_hours: row.interval_hours,
        cache: row.cache,
        last_fetch_status: row.last_fetch_status,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// 取 profile 的 token(同时校验 profile 存在);不存在 → 404。
async fn profile_token(state: &AppState, profile_id: &str) -> ApiResult<String> {
    sqlx::query_scalar::<_, String>("SELECT token FROM profiles WHERE id = ?")
        .bind(profile_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)
}

async fn fetch_one(state: &AppState, profile_id: &str, id: &str) -> ApiResult<RuleSetRow> {
    sqlx::query_as::<_, RuleSetRow>(&format!(
        "SELECT {META_COLS} FROM profile_rule_sets WHERE id = ? AND profile_id = ?"
    ))
    .bind(id)
    .bind(profile_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)
}

// ─── CRUD ──────────────────────────────────────────────────────────────────────

/// `GET /api/profiles/:id/rule-sets` —— 列出该订阅的规则集。
pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let token = profile_token(&state, &profile_id).await?;
    let rows = sqlx::query_as::<_, RuleSetRow>(&format!(
        "SELECT {META_COLS} FROM profile_rule_sets WHERE profile_id = ? ORDER BY name ASC"
    ))
    .bind(&profile_id)
    .fetch_all(&state.db)
    .await?;
    let out: Vec<ProfileRuleSetResponse> = rows
        .into_iter()
        .map(|row| to_response(&state, &token, row))
        .collect();
    Ok(Json(out))
}

/// `POST /api/profiles/:id/rule-sets` —— 新建。name 在本订阅内唯一冲突 → 409。
pub async fn create(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
    Json(body): Json<RuleSetBody>,
) -> ApiResult<impl IntoResponse> {
    let token = profile_token(&state, &profile_id).await?;
    let n = rulelib::normalize(&body, None)?;
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO profile_rule_sets (id, profile_id, name, behavior, format, source, content, \
         rule_count, url, interval_hours, cache, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&profile_id)
    .bind(body.name.trim())
    .bind(&body.behavior)
    .bind(&body.format)
    .bind(&n.source)
    .bind(&n.content)
    .bind(n.rule_count)
    .bind(&n.url)
    .bind(n.interval_hours)
    .bind(n.cache)
    .bind(body.enabled.unwrap_or(true))
    .bind(&ts)
    .bind(&ts)
    .execute(&state.db)
    .await?;
    let row = fetch_one(&state, &profile_id, &id).await?;
    Ok((StatusCode::CREATED, Json(to_response(&state, &token, row))))
}

/// `PUT /api/profiles/:id/rule-sets/:rsid` —— 更新。任何更新都清空镜像缓存(下次拉取重新回源)。
pub async fn update(
    State(state): State<Arc<AppState>>,
    Path((profile_id, rsid)): Path<(String, String)>,
    Json(body): Json<RuleSetBody>,
) -> ApiResult<impl IntoResponse> {
    let token = profile_token(&state, &profile_id).await?;
    let existing = fetch_one(&state, &profile_id, &rsid).await?;
    let n = rulelib::normalize(&body, existing.url.as_deref())?;
    sqlx::query(
        "UPDATE profile_rule_sets SET name = ?, behavior = ?, format = ?, source = ?, content = ?, \
         rule_count = ?, url = ?, interval_hours = ?, cache = ?, enabled = ?, \
         cached_body = NULL, cached_at = NULL, last_fetch_status = NULL, updated_at = ?
         WHERE id = ? AND profile_id = ?",
    )
    .bind(body.name.trim())
    .bind(&body.behavior)
    .bind(&body.format)
    .bind(&n.source)
    .bind(&n.content)
    .bind(n.rule_count)
    .bind(&n.url)
    .bind(n.interval_hours)
    .bind(n.cache)
    .bind(body.enabled.unwrap_or(true))
    .bind(now())
    .bind(&rsid)
    .bind(&profile_id)
    .execute(&state.db)
    .await?;
    let row = fetch_one(&state, &profile_id, &rsid).await?;
    Ok(Json(to_response(&state, &token, row)))
}

/// `DELETE /api/profiles/:id/rule-sets/:rsid` —— 删除(只删定义;引用它的规则行另行删除)。
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path((profile_id, rsid)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    let result = sqlx::query("DELETE FROM profile_rule_sets WHERE id = ? AND profile_id = ?")
        .bind(&rsid)
        .bind(&profile_id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// ─── 从 ② 全局库导入 ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ImportBody {
    /// 要从全局「规则托管」库复制进本订阅的规则集名。
    names: Vec<String>,
    /// 复制后追加的 `RULE-SET` 规则统一指向的策略。
    policy: String,
}

#[derive(Serialize)]
pub struct ImportResult {
    imported: usize,
}

/// `POST /api/profiles/:id/rule-sets/import` —— 从全局 ② 复制选中规则集进本订阅 ③(含真实远程 URL,
/// 前端拿不到脱敏后的 URL,故由后端复制),并为尚未引用的名追加 `RULE-SET,<name>,<policy>` 规则行,
/// 随后重缝缓存使其立即生效。已存在的定义/已引用的规则行跳过。
pub async fn import(
    State(state): State<Arc<AppState>>,
    Path(profile_id): Path<String>,
    Json(body): Json<ImportBody>,
) -> ApiResult<impl IntoResponse> {
    let _ = profile_token(&state, &profile_id).await?;

    // 复制定义:逐个把 ② 行复制进 ③(本订阅已有同名则跳过)。
    for name in dedup(&body.names) {
        let exists = sqlx::query_scalar::<_, String>(
            "SELECT id FROM profile_rule_sets WHERE profile_id = ? AND name = ?",
        )
        .bind(&profile_id)
        .bind(&name)
        .fetch_optional(&state.db)
        .await?;
        if exists.is_some() {
            continue;
        }
        let global = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                Option<String>,
                i64,
                bool,
            ),
        >(
            "SELECT name, behavior, format, source, content, url, interval_hours, cache \
             FROM rule_sets WHERE name = ?",
        )
        .bind(&name)
        .fetch_optional(&state.db)
        .await?;
        let Some((name, behavior, format, source, content, url, interval_hours, cache)) = global
        else {
            continue;
        };
        let rule_count = if source == "manual" {
            rulelib::payload_count(&content)
        } else {
            0
        };
        let id = uuid::Uuid::new_v4().to_string();
        let ts = now();
        sqlx::query(
            "INSERT INTO profile_rule_sets (id, profile_id, name, behavior, format, source, content, \
             rule_count, url, interval_hours, cache, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
        )
        .bind(&id)
        .bind(&profile_id)
        .bind(&name)
        .bind(&behavior)
        .bind(&format)
        .bind(&source)
        .bind(&content)
        .bind(rule_count)
        .bind(&url)
        .bind(interval_hours)
        .bind(cache)
        .bind(&ts)
        .bind(&ts)
        .execute(&state.db)
        .await?;
    }

    // 追加规则行:仅为尚未被 RULE-SET 引用的名追加,统一指向 `policy`。
    let content =
        sqlx::query_scalar::<_, String>("SELECT content FROM rulesets WHERE profile_id = ?")
            .bind(&profile_id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_default();
    let referenced = crate::converter::ruleset_refs(&content);
    let policy = body.policy.trim();
    let mut lines: Vec<String> = Vec::new();
    for name in dedup(&body.names) {
        if !referenced.iter().any(|r| r == &name) {
            lines.push(format!("RULE-SET,{name},{policy}"));
        }
    }
    let imported = lines.len();
    if imported > 0 {
        let mut next = content.clone();
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(&lines.join("\n"));
        sqlx::query("UPDATE rulesets SET content = ?, updated_at = ? WHERE profile_id = ?")
            .bind(&next)
            .bind(now())
            .bind(&profile_id)
            .execute(&state.db)
            .await?;
        // 规则完全由用户定义,就地重缝缓存使编辑立即反映到所服务的订阅。
        if crate::generate::resync_cache(&state, &profile_id)
            .await
            .is_err()
        {
            tracing::warn!(profile = %profile_id, "failed to resync cache after rule-set import");
        }
    }

    Ok(Json(ImportResult { imported }))
}

fn dedup(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n = n.trim();
        if !n.is_empty() && !out.iter().any(|x| x == n) {
            out.push(n.to_string());
        }
    }
    out
}

// ─── 公开托管端点 ───────────────────────────────────────────────────────────────

/// 托管渲染所需的行(含 BLOB `cached_body`)。
#[derive(FromRow)]
struct ServeRow {
    id: String,
    behavior: String,
    format: String,
    source: String,
    content: String,
    url: Option<String>,
    interval_hours: i64,
    cache: bool,
    cached_body: Option<Vec<u8>>,
    cached_at: Option<String>,
}

const SERVE_COLS: &str =
    "id, behavior, format, source, content, url, interval_hours, cache, cached_body, cached_at";

/// `GET /:public_path_prefix/api/sub/:token/r/:name/:file` —— 公开托管本订阅的规则集内容。无鉴权;
/// 按 token→订阅、再按 `(profile_id, name)` 定位,统一 404(前缀错 / token 错 / 名不存在 / 未启用 /
/// 文件名不符 / remote 未托管 一律 404)。`:file` 必须等于 `<behavior>.<format>`。规则集是规则清单、
/// 非私密,按名可枚举可接受(见 `docs/security-design.md`),由 IP 限流抑制枚举。
pub async fn public_serve(
    State(state): State<Arc<AppState>>,
    Path((prefix, token, name, file)): Path<(String, String, String, String)>,
) -> Response {
    if prefix != state.current_prefix() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let profile_id =
        match sqlx::query_scalar::<_, String>("SELECT id FROM profiles WHERE token = ?")
            .bind(&token)
            .fetch_optional(&state.db)
            .await
        {
            Ok(Some(id)) => id,
            _ => return StatusCode::NOT_FOUND.into_response(),
        };
    let row = match fetch_serve_by_name(&state, &profile_id, &name).await {
        Ok(Some(r)) => r,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if file != format!("{}.{}", row.behavior, row.format) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match row.source.as_str() {
        "manual" => rulelib::serve_bytes(
            &row.format,
            rulelib::render_manual(&row.content, &row.format).into_bytes(),
        ),
        // remote + cache:面板二次托管;cache 关时不托管(转换时直接注入上游 URL)→ 404。
        "remote" if row.cache => serve_remote(&state, row).await,
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn fetch_serve_by_name(
    state: &AppState,
    profile_id: &str,
    name: &str,
) -> ApiResult<Option<ServeRow>> {
    Ok(sqlx::query_as::<_, ServeRow>(&format!(
        "SELECT {SERVE_COLS} FROM profile_rule_sets WHERE profile_id = ? AND name = ? AND enabled = 1"
    ))
    .bind(profile_id)
    .bind(name)
    .fetch_optional(&state.db)
    .await?)
}

async fn fetch_serve_by_id(state: &AppState, id: &str) -> ApiResult<Option<ServeRow>> {
    Ok(sqlx::query_as::<_, ServeRow>(&format!(
        "SELECT {SERVE_COLS} FROM profile_rule_sets WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(&state.db)
    .await?)
}

/// 远程镜像的懒刷新托管:单飞合并并发;缓存超 `interval_hours` 才回源拉取(SSRF 安全字节);拉取失败
/// 回退旧缓存,无缓存则 `503`。与全局库 `rule_sets::serve_remote` 同构,但作用于 per-profile 表。
async fn serve_remote(state: &AppState, row: ServeRow) -> Response {
    let Some(url) = row.url.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let lock = state
        .single_flight
        .lock_for(&format!("profile-ruleset:{}", row.id));
    let _guard = lock.lock().await;

    // 等锁期间可能已被另一个请求刷新过——重读最新行。
    let row = match fetch_serve_by_id(state, &row.id).await {
        Ok(Some(r)) => r,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if let (Some(body), Some(at)) = (&row.cached_body, &row.cached_at) {
        if rulelib::is_fresh(at, row.interval_hours) {
            return rulelib::serve_bytes(&row.format, body.clone());
        }
    }

    match state.fetcher.fetch_bytes(&url).await {
        Ok(bytes) => {
            let count = rulelib::body_count(&bytes, &row.format);
            let _ = persist_remote_cache(state, &row.id, &bytes, count).await;
            rulelib::serve_bytes(&row.format, bytes)
        }
        Err(e) => {
            let _ = update_fetch_status(state, &row.id, &e.status_label()).await;
            match row.cached_body {
                Some(b) => rulelib::serve_bytes(&row.format, b), // 回退旧缓存
                None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            }
        }
    }
}

async fn persist_remote_cache(
    state: &AppState,
    id: &str,
    bytes: &[u8],
    count: i64,
) -> ApiResult<()> {
    sqlx::query(
        "UPDATE profile_rule_sets SET cached_body = ?, cached_at = ?, last_fetch_status = 'success', \
         rule_count = ? WHERE id = ?",
    )
    .bind(bytes)
    .bind(now())
    .bind(count)
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn update_fetch_status(state: &AppState, id: &str, label: &str) -> ApiResult<()> {
    sqlx::query("UPDATE profile_rule_sets SET last_fetch_status = ? WHERE id = ?")
        .bind(label)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(())
}
