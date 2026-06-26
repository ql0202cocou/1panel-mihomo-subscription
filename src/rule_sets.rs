//! 全局规则集托管(「规则配置」):跨订阅复用的命名规则集库。面板把每个规则集托管在
//! `/<prefix>/r/<name>/<behavior>.<format>`,订阅以 `RULE-SET,<name>` 引用,转换时由
//! `src/generate.rs` 注入到输出的 `rule-providers:`。提供增删改 + 全局排序 + 公开托管端点。
//! 契约见 `docs/api-design.md`。
//!
//! 两种来源:`manual` 托管管理员录入的 payload;`remote` 镜像远程规则集(`cache=1` 时面板按
//! `interval_hours` 懒刷新、二次托管,二进制 `mrs` 也支持;`cache=0` 则不托管,转换时直接注入上游
//! URL)。远程拉取走 SSRF 安全的 `state.fetcher.fetch_bytes`,单飞合并并发、失败回退旧缓存。

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use sqlx::FromRow;

use crate::app::AppState;
use crate::error::{ApiError, ApiResult};
use crate::mask;
use crate::util::now;

/// 排序请求的上界,与其它排序端点保持一致。
const MAX_ORDER_ENTRIES: usize = 5_000;
const MAX_ORDER_NAME_LEN: usize = 256;
/// 规则集名长度上限(同时是 URL 路径段与 `RULE-SET` 引用名)。
const MAX_NAME_LEN: usize = 128;
const BEHAVIORS: &[&str] = &["domain", "ipcidr", "classical"];
const MANUAL_FORMATS: &[&str] = &["yaml", "text"];
const REMOTE_FORMATS: &[&str] = &["yaml", "text", "mrs"];

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

#[derive(Serialize)]
pub struct RuleSetResponse {
    id: String,
    name: String,
    behavior: String,
    format: String,
    source: String,
    content: String,
    enabled: bool,
    /// 规则条数(manual=payload 行数;remote=最近成功镜像的行数,mrs 为 0)。
    count: i64,
    /// 面板托管链接。
    url: String,
    /// 远程来源 URL(已脱敏);manual 为 null。
    remote_url_masked: Option<String>,
    interval_hours: i64,
    cache: bool,
    last_fetch_status: Option<String>,
    created_at: String,
    updated_at: String,
}

const META_COLS: &str = "id, name, behavior, format, source, content, rule_count, url, \
     interval_hours, cache, last_fetch_status, enabled, created_at, updated_at";

fn rule_set_response(state: &AppState, row: RuleSetRow) -> RuleSetResponse {
    RuleSetResponse {
        url: state.rule_set_url(&row.name, &row.behavior, &row.format),
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

/// 有效 payload 行:非空、非 `#` 注释。供计数与手动托管渲染共用。
fn payload_lines(content: &str) -> impl Iterator<Item = &str> {
    content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn payload_count(content: &str) -> i64 {
    payload_lines(content).count() as i64
}

/// 远程镜像体的近似规则数:文本格式按有效行数,`mrs`(二进制)为 0。
fn body_count(bytes: &[u8], format: &str) -> i64 {
    if format == "mrs" {
        0
    } else {
        payload_count(&String::from_utf8_lossy(bytes))
    }
}

// ─── 请求体 / 校验 ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RuleSetBody {
    name: String,
    behavior: String,
    format: String,
    source: Option<String>,
    content: Option<String>,
    url: Option<String>,
    interval_hours: Option<i64>,
    cache: Option<bool>,
    enabled: Option<bool>,
}

struct Normalized {
    source: String,
    content: String,
    url: Option<String>,
    interval_hours: i64,
    cache: bool,
    rule_count: i64,
}

/// 校验并归一化请求体。name 入 URL 路径 + 作 `RULE-SET` 引用名,故限定安全字符集;manual/remote
/// 各有合法的 format 集合,remote 必须给可拉取的 http(s) URL。`existing_url` 是更新时已存的远程
/// URL:remote 编辑留空则沿用它(URL 已脱敏不回显,与 profile 一致,改地址需重填)。
fn normalize(body: &RuleSetBody, existing_url: Option<&str>) -> ApiResult<Normalized> {
    let name = body.name.trim();
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(ApiError::BadRequest(format!(
            "name is required and must be at most {MAX_NAME_LEN} bytes"
        )));
    }
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(ApiError::BadRequest(
            "name may only contain letters, digits, '.', '_', '-'".into(),
        ));
    }
    if !BEHAVIORS.contains(&body.behavior.as_str()) {
        return Err(ApiError::BadRequest(
            "behavior must be one of domain, ipcidr, classical".into(),
        ));
    }

    let source = body.source.as_deref().unwrap_or("manual");
    match source {
        "manual" => {
            if !MANUAL_FORMATS.contains(&body.format.as_str()) {
                return Err(ApiError::BadRequest(
                    "manual format must be yaml or text".into(),
                ));
            }
            let content = body.content.clone().unwrap_or_default();
            Ok(Normalized {
                source: "manual".into(),
                rule_count: payload_count(&content),
                content,
                url: None,
                interval_hours: 24,
                cache: true,
            })
        }
        "remote" => {
            if !REMOTE_FORMATS.contains(&body.format.as_str()) {
                return Err(ApiError::BadRequest(
                    "remote format must be yaml, text or mrs".into(),
                ));
            }
            // 优先用本次填写的 URL;留空则沿用已存的(编辑场景)。
            let provided = body.url.as_deref().map(str::trim).filter(|u| !u.is_empty());
            let url = match provided {
                Some(u) => {
                    if !(u.starts_with("http://") || u.starts_with("https://")) {
                        return Err(ApiError::BadRequest(
                            "remote source requires an http(s) url".into(),
                        ));
                    }
                    u.to_string()
                }
                None => existing_url
                    .map(str::trim)
                    .filter(|u| !u.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        ApiError::BadRequest("remote source requires an http(s) url".into())
                    })?,
            };
            let interval_hours = body.interval_hours.unwrap_or(24);
            if interval_hours < 1 {
                return Err(ApiError::BadRequest("interval_hours must be >= 1".into()));
            }
            Ok(Normalized {
                source: "remote".into(),
                content: String::new(),
                url: Some(url),
                interval_hours,
                cache: body.cache.unwrap_or(true),
                rule_count: 0, // 首次成功镜像后回填
            })
        }
        _ => Err(ApiError::BadRequest(
            "source must be manual or remote".into(),
        )),
    }
}

// ─── CRUD ──────────────────────────────────────────────────────────────────────

async fn fetch_all(state: &AppState) -> ApiResult<Vec<RuleSetRow>> {
    Ok(sqlx::query_as::<_, RuleSetRow>(&format!(
        "SELECT {META_COLS} FROM rule_sets ORDER BY position ASC, name ASC"
    ))
    .fetch_all(&state.db)
    .await?)
}

async fn fetch_one(state: &AppState, id: &str) -> ApiResult<RuleSetRow> {
    sqlx::query_as::<_, RuleSetRow>(&format!("SELECT {META_COLS} FROM rule_sets WHERE id = ?"))
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(ApiError::NotFound)
}

/// `GET /api/rule-sets` —— 按库内顺序列出全部规则集。
pub async fn list(State(state): State<Arc<AppState>>) -> ApiResult<impl IntoResponse> {
    let sets: Vec<RuleSetResponse> = fetch_all(&state)
        .await?
        .into_iter()
        .map(|row| rule_set_response(&state, row))
        .collect();
    Ok(Json(sets))
}

/// `POST /api/rule-sets` —— 新建规则集,落在库末尾。name 唯一冲突 → 409。
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RuleSetBody>,
) -> ApiResult<impl IntoResponse> {
    let n = normalize(&body, None)?;
    let id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    let position =
        sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(position), -1) + 1 FROM rule_sets")
            .fetch_one(&state.db)
            .await?;
    sqlx::query(
        "INSERT INTO rule_sets (id, name, behavior, format, source, content, rule_count, url, \
         interval_hours, cache, enabled, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
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
    .bind(position)
    .bind(&ts)
    .bind(&ts)
    .execute(&state.db)
    .await?;
    let row = fetch_one(&state, &id).await?;
    Ok((StatusCode::CREATED, Json(rule_set_response(&state, row))))
}

/// `PUT /api/rule-sets/:id` —— 更新规则集。任何更新都清空镜像缓存(下次拉取重新回源)。
pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RuleSetBody>,
) -> ApiResult<impl IntoResponse> {
    let existing = fetch_one(&state, &id).await?;
    let n = normalize(&body, existing.url.as_deref())?;
    sqlx::query(
        "UPDATE rule_sets SET name = ?, behavior = ?, format = ?, source = ?, content = ?, \
         rule_count = ?, url = ?, interval_hours = ?, cache = ?, enabled = ?, \
         cached_body = NULL, cached_at = NULL, last_fetch_status = NULL, updated_at = ?
         WHERE id = ?",
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
    .bind(&id)
    .execute(&state.db)
    .await?;
    let row = fetch_one(&state, &id).await?;
    Ok(Json(rule_set_response(&state, row)))
}

/// `DELETE /api/rule-sets/:id` —— 从库中删除规则集。
pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let result = sqlx::query("DELETE FROM rule_sets WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ReorderBody {
    /// 期望的库内顺序(规则集名)。不在库中的名字忽略;库中未列出的保持原有相对顺序、排在已列出之后。
    order: Vec<String>,
}

/// `PUT /api/rule-sets/order` —— 设置库内显示顺序。仅影响「规则配置」页的展示次序,不影响任何
/// 订阅的输出(rule-providers 是按引用注入的 map,顺序无语义),故无需重缝缓存。
pub async fn set_order(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ReorderBody>,
) -> ApiResult<impl IntoResponse> {
    if body.order.len() > MAX_ORDER_ENTRIES {
        return Err(ApiError::BadRequest(format!(
            "order must have at most {MAX_ORDER_ENTRIES} entries"
        )));
    }
    if body.order.iter().any(|n| n.len() > MAX_ORDER_NAME_LEN) {
        return Err(ApiError::BadRequest(format!(
            "names must be at most {MAX_ORDER_NAME_LEN} bytes"
        )));
    }

    let mut names: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT name FROM rule_sets ORDER BY position ASC, name ASC",
    )
    .fetch_all(&state.db)
    .await?;
    crate::converter::reorder_by_name(&mut names, |s| Some(s.as_str()), &body.order);

    let mut tx = state.db.begin().await?;
    for (position, name) in names.iter().enumerate() {
        sqlx::query("UPDATE rule_sets SET position = ? WHERE name = ?")
            .bind(position as i64)
            .bind(name)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── 公开托管端点 ───────────────────────────────────────────────────────────────

/// 托管渲染所需的行(含 BLOB `cached_body`,故与列表查询分开,避免列表读 BLOB)。
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

/// `GET /:public_path_prefix/r/:name/:file` —— 公开托管规则集内容。无鉴权;按名定位,统一 404
/// (前缀错 / 名不存在 / 未启用 / 文件名不符 / remote 未托管 一律 404)。`:file` 必须等于
/// `<behavior>.<format>`。规则集是规则清单、非私密,按名可枚举可接受(见 `docs/security-design.md`)。
pub async fn public_serve(
    State(state): State<Arc<AppState>>,
    Path((prefix, name, file)): Path<(String, String, String)>,
) -> Response {
    if prefix != state.current_prefix() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let row = match fetch_serve_by_name(&state, &name).await {
        Ok(Some(r)) => r,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if file != format!("{}.{}", row.behavior, row.format) {
        return StatusCode::NOT_FOUND.into_response();
    }
    match row.source.as_str() {
        "manual" => serve_bytes(
            &row.format,
            render_manual(&row.content, &row.format).into_bytes(),
        ),
        // remote + cache:面板二次托管;cache 关时不托管(转换时直接注入上游 URL)→ 404。
        "remote" if row.cache => serve_remote(&state, row).await,
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn fetch_serve_by_name(state: &AppState, name: &str) -> ApiResult<Option<ServeRow>> {
    Ok(sqlx::query_as::<_, ServeRow>(&format!(
        "SELECT {SERVE_COLS} FROM rule_sets WHERE name = ? AND enabled = 1"
    ))
    .bind(name)
    .fetch_optional(&state.db)
    .await?)
}

async fn fetch_serve_by_id(state: &AppState, id: &str) -> ApiResult<Option<ServeRow>> {
    Ok(
        sqlx::query_as::<_, ServeRow>(&format!("SELECT {SERVE_COLS} FROM rule_sets WHERE id = ?"))
            .bind(id)
            .fetch_optional(&state.db)
            .await?,
    )
}

/// 远程镜像的懒刷新托管:单飞合并并发;缓存超 `interval_hours` 才回源拉取(SSRF 安全字节);
/// 拉取失败回退旧缓存,无缓存则 `503`。复用 `generate::serve_or_refresh` 的设计。
async fn serve_remote(state: &AppState, row: ServeRow) -> Response {
    let Some(url) = row.url.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let lock = state.single_flight.lock_for(&format!("ruleset:{}", row.id));
    let _guard = lock.lock().await;

    // 等锁期间可能已被另一个请求刷新过——重读最新行。
    let row = match fetch_serve_by_id(state, &row.id).await {
        Ok(Some(r)) => r,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if let (Some(body), Some(at)) = (&row.cached_body, &row.cached_at) {
        if is_fresh(at, row.interval_hours) {
            return serve_bytes(&row.format, body.clone());
        }
    }

    match state.fetcher.fetch_bytes(&url).await {
        Ok(bytes) => {
            let count = body_count(&bytes, &row.format);
            let _ = persist_remote_cache(state, &row.id, &bytes, count).await;
            serve_bytes(&row.format, bytes)
        }
        Err(e) => {
            let _ = update_fetch_status(state, &row.id, &e.status_label()).await;
            match row.cached_body {
                Some(b) => serve_bytes(&row.format, b), // 回退旧缓存
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
        "UPDATE rule_sets SET cached_body = ?, cached_at = ?, last_fetch_status = 'success', \
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
    sqlx::query("UPDATE rule_sets SET last_fetch_status = ? WHERE id = ?")
        .bind(label)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// 缓存是否仍在 `interval_hours` 新鲜窗口内。
fn is_fresh(cached_at: &str, interval_hours: i64) -> bool {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(cached_at) else {
        return false;
    };
    let age = chrono::Utc::now().signed_duration_since(at.with_timezone(&chrono::Utc));
    age < chrono::Duration::hours(interval_hours.max(1))
}

/// 渲染手动规则集:`yaml` → Mihomo rule-provider 的 `payload:` 列表;`text` → 逐行原样。
fn render_manual(content: &str, format: &str) -> String {
    if format == "yaml" {
        let payload: Vec<Value> = payload_lines(content).map(Value::from).collect();
        let mut map = Mapping::new();
        map.insert(Value::from("payload"), Value::Sequence(payload));
        serde_yaml::to_string(&Value::Mapping(map)).unwrap_or_default()
    } else {
        let mut s = payload_lines(content).collect::<Vec<_>>().join("\n");
        if !s.is_empty() {
            s.push('\n');
        }
        s
    }
}

/// 按 format 选 content-type 返回字节体(`mrs` 二进制 → octet-stream)。
fn serve_bytes(format: &str, bytes: Vec<u8>) -> Response {
    let ct = if format == "mrs" {
        "application/octet-stream"
    } else {
        "text/plain; charset=utf-8"
    };
    ([(header::CONTENT_TYPE, ct)], bytes).into_response()
}
