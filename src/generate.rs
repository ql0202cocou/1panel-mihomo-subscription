//! Generation, preview, and the public subscription endpoint.
//!
//! `generate` (also the source-card manual refresh) fetches the provider,
//! converts, persists the cache, and updates `last_fetch_*`. `preview` is the
//! read-only counterpart (no cache write, no `last_fetch_*` change). The public
//! endpoint serves fresh cache, refreshes under a per-profile single-flight
//! lock, falls back to stale cache on refresh failure, and returns a uniform
//! `404` for invalid access or a generic `503` when no cache exists and the
//! fetch fails. See `docs/api-design.md` and `docs/security-design.md`.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use subtle::ConstantTimeEq;

use crate::app::AppState;
use crate::converter::{self, ConvertError, ConvertInput, CustomGroup, CustomNode};
use crate::error::{ApiError, ApiResult};
use crate::util::now;

const UPDATE_INTERVAL_HOURS: u32 = 24;

// ─── Shared row/input types ─────────────────────────────────────────────────

#[derive(FromRow, Clone)]
struct ProfileCore {
    id: String,
    name: String,
    source_url: String,
    token: String,
    enabled: bool,
}

#[derive(FromRow, Clone)]
struct CacheRow {
    output_yaml: String,
    subscription_userinfo: Option<String>,
    generated_at: String,
}

struct Built {
    yaml: String,
    userinfo: Option<String>,
    content_hash: String,
    generated_at: String,
}

/// Outcome of a refresh attempt, used to choose the public response.
enum BuildError {
    Validation(Vec<String>),
    Upstream(String),
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GenerateResponse {
    subscription_url: String,
    generated_at: String,
}

/// `POST /api/profiles/:id/generate` — validate, fetch, convert, persist.
pub async fn generate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let profile = load_core(&state, &id).await?.ok_or(ApiError::NotFound)?;

    let built = match fetch_and_convert(&state, &profile).await {
        Ok(b) => b,
        Err(BuildError::Validation(errors)) => return Err(ApiError::Validation(errors)),
        Err(BuildError::Upstream(label)) => return Err(ApiError::Upstream(label)),
    };

    persist_cache(&state, &profile.id, &built).await?;
    Ok(Json(GenerateResponse {
        subscription_url: state.subscription_url(&profile.token),
        generated_at: built.generated_at,
    }))
}

/// `GET /api/profiles/:id/preview` — read-only generated YAML. Returns fresh
/// cache if present, otherwise generates live without persisting or touching
/// `last_fetch_*`.
pub async fn preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let profile = load_core(&state, &id).await?.ok_or(ApiError::NotFound)?;

    if let Some(cache) = load_cache(&state, &profile.id).await? {
        if is_fresh(&cache.generated_at, state.cache_ttl) {
            return Ok(yaml_body(cache.output_yaml));
        }
    }

    // Live generation; do not persist and do not update last_fetch_*.
    let fetched = state
        .fetcher
        .fetch(&profile.source_url)
        .await
        .map_err(|e| ApiError::Upstream(e.status_label()))?;
    let yaml = convert(&state, &profile.id, &fetched.body)
        .await?
        .map_err(map_convert_err)?;
    Ok(yaml_body(yaml))
}

#[derive(Serialize)]
struct ProviderRules {
    rules: Vec<String>,
}

/// `GET /api/profiles/:id/provider-rules` — fetch the provider subscription and
/// return its `rules` lines, so the admin can seed the rule editor with the
/// airport's own rules (which the converter otherwise replaces). Live,
/// SSRF-protected fetch; not cached and does not touch `last_fetch_*`.
pub async fn provider_rules(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let profile = load_core(&state, &id).await?.ok_or(ApiError::NotFound)?;
    let fetched = state
        .fetcher
        .fetch(&profile.source_url)
        .await
        .map_err(|e| ApiError::Upstream(e.status_label()))?;
    let root = crate::yaml::parse_limited(&fetched.body)
        .map_err(|_| ApiError::Upstream("provider_parse".to_string()))?;
    let rules = match root.get("rules") {
        Some(serde_yaml::Value::Sequence(items)) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    Ok(Json(ProviderRules { rules }).into_response())
}

/// `GET /:public_path_prefix/api/sub/:token` — public subscription download.
pub async fn public_sub(
    State(state): State<Arc<AppState>>,
    Path((prefix, token)): Path<(String, String)>,
) -> Response {
    // Always perform the token lookup regardless of whether the prefix matched,
    // and compare the prefix in constant time, so response timing cannot
    // confirm the path prefix independently (see security-design.md).
    let prefix_ok: bool = prefix
        .as_bytes()
        .ct_eq(state.current_prefix().as_bytes())
        .into();
    let profile = load_core_by_token(&state, &token).await.ok().flatten();

    let access_ok = prefix_ok && profile.as_ref().is_some_and(|p| p.enabled);
    if !access_ok {
        return StatusCode::NOT_FOUND.into_response();
    }
    let profile = profile.expect("checked by access_ok");

    match serve_or_refresh(&state, &profile).await {
        Some(served) => public_response(&profile.name, served),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

// ─── Public serve/refresh with single-flight ────────────────────────────────

struct Served {
    yaml: String,
    userinfo: Option<String>,
}

async fn serve_or_refresh(state: &AppState, profile: &ProfileCore) -> Option<Served> {
    if let Ok(Some(cache)) = load_cache(state, &profile.id).await {
        if is_fresh(&cache.generated_at, state.cache_ttl) {
            return Some(cache.into());
        }
    }

    // Coalesce concurrent refreshes of this profile.
    let lock = state.single_flight.lock_for(&profile.id);
    let _guard = lock.lock().await;

    // Re-check: another request may have refreshed while we waited.
    let stale = load_cache(state, &profile.id).await.ok().flatten();
    if let Some(cache) = &stale {
        if is_fresh(&cache.generated_at, state.cache_ttl) {
            return Some(cache.clone().into());
        }
    }

    match fetch_and_convert(state, profile).await {
        Ok(built) => {
            if persist_cache(state, &profile.id, &built).await.is_err() {
                tracing::error!(profile = %profile.id, "failed to persist generated cache");
            }
            Some(Served {
                yaml: built.yaml,
                userinfo: built.userinfo,
            })
        }
        Err(err) => {
            if let BuildError::Upstream(label) = &err {
                let _ = update_last_fetch(state, &profile.id, label).await;
            }
            // Serve stale cache if we have it; otherwise signal 503.
            match stale {
                Some(cache) => {
                    tracing::warn!(profile = %profile.id, "refresh failed; serving stale cache");
                    Some(cache.into())
                }
                None => None,
            }
        }
    }
}

impl From<CacheRow> for Served {
    fn from(c: CacheRow) -> Self {
        Served {
            yaml: c.output_yaml,
            userinfo: c.subscription_userinfo,
        }
    }
}

// ─── Core fetch + convert ───────────────────────────────────────────────────

/// Fetch the provider and convert. On a successful fetch, updates
/// `last_fetch_*` to `success`; on fetch failure, records the status label.
async fn fetch_and_convert(state: &AppState, profile: &ProfileCore) -> Result<Built, BuildError> {
    let fetched = match state.fetcher.fetch(&profile.source_url).await {
        Ok(f) => f,
        Err(e) => {
            let label = e.status_label();
            let _ = update_last_fetch(state, &profile.id, &label).await;
            return Err(BuildError::Upstream(label));
        }
    };
    let _ = update_last_fetch(state, &profile.id, "success").await;

    let yaml = convert(state, &profile.id, &fetched.body)
        .await
        .map_err(|_| BuildError::Upstream("internal".to_string()))?
        .map_err(|e| match e {
            ConvertError::Validation(v) => BuildError::Validation(v),
            ConvertError::ProviderParse => BuildError::Upstream("provider_parse".to_string()),
        })?;

    let content_hash = hash_inputs(&fetched.body, &yaml);
    Ok(Built {
        yaml,
        userinfo: fetched.subscription_userinfo,
        content_hash,
        generated_at: now(),
    })
}

/// Load convert inputs from the DB and run the converter. The outer
/// `ApiResult` is for DB errors; the inner `Result` is the converter outcome.
async fn convert(
    state: &AppState,
    profile_id: &str,
    provider_yaml: &str,
) -> ApiResult<Result<String, ConvertError>> {
    let rules =
        sqlx::query_scalar::<_, String>("SELECT content FROM rulesets WHERE profile_id = ?")
            .bind(profile_id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_default();

    let nodes = sqlx::query_as::<_, (String, String)>(
        "SELECT name, content FROM custom_nodes WHERE profile_id = ? AND enabled = 1 ORDER BY created_at",
    )
    .bind(profile_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(name, content)| CustomNode { name, content })
    .collect();

    let groups = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT name, group_type, members, options FROM custom_groups WHERE profile_id = ? AND enabled = 1 ORDER BY created_at",
    )
    .bind(profile_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(name, group_type, members, options)| CustomGroup {
        name,
        group_type,
        members: serde_json::from_str(&members).unwrap_or_default(),
        options: options.and_then(|o| serde_json::from_str(&o).ok()),
    })
    .collect();

    // Manual proxy / proxy-group ordering (NULL/garbage -> empty -> default).
    let (node_order, group_order) = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT node_order, group_order FROM profiles WHERE id = ?",
    )
    .bind(profile_id)
    .fetch_optional(&state.db)
    .await?
    .map(|(n, g)| (parse_order(n), parse_order(g)))
    .unwrap_or_default();

    Ok(converter::convert(ConvertInput {
        provider_yaml,
        rules: &rules,
        nodes,
        groups,
        node_order,
        group_order,
    }))
}

/// Parse a stored `node_order`/`group_order` JSON array; NULL or malformed
/// values yield an empty list (= default order).
fn parse_order(stored: Option<String>) -> Vec<String> {
    stored
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

fn map_convert_err(e: ConvertError) -> ApiError {
    match e {
        ConvertError::Validation(v) => ApiError::Validation(v),
        ConvertError::ProviderParse => ApiError::Upstream("provider_parse".to_string()),
    }
}

// ─── DB helpers ─────────────────────────────────────────────────────────────

async fn load_core(state: &AppState, id: &str) -> ApiResult<Option<ProfileCore>> {
    Ok(sqlx::query_as::<_, ProfileCore>(
        "SELECT id, name, source_url, token, enabled FROM profiles WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?)
}

async fn load_core_by_token(state: &AppState, token: &str) -> ApiResult<Option<ProfileCore>> {
    Ok(sqlx::query_as::<_, ProfileCore>(
        "SELECT id, name, source_url, token, enabled FROM profiles WHERE token = ?",
    )
    .bind(token)
    .fetch_optional(&state.db)
    .await?)
}

async fn load_cache(state: &AppState, profile_id: &str) -> ApiResult<Option<CacheRow>> {
    Ok(sqlx::query_as::<_, CacheRow>(
        "SELECT output_yaml, subscription_userinfo, generated_at FROM generated_cache WHERE profile_id = ?",
    )
    .bind(profile_id)
    .fetch_optional(&state.db)
    .await?)
}

async fn persist_cache(state: &AppState, profile_id: &str, built: &Built) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO generated_cache (profile_id, content_hash, output_yaml, subscription_userinfo, generated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(profile_id) DO UPDATE SET
            content_hash = excluded.content_hash,
            output_yaml = excluded.output_yaml,
            subscription_userinfo = excluded.subscription_userinfo,
            generated_at = excluded.generated_at",
    )
    .bind(profile_id)
    .bind(&built.content_hash)
    .bind(&built.yaml)
    .bind(&built.userinfo)
    .bind(&built.generated_at)
    .execute(&state.db)
    .await?;

    // Snapshot the output's proxy/group name order so it stays stable across
    // provider refreshes: a node/group that still exists keeps its slot (its
    // info is refreshed by name from the new provider YAML), and any newly added
    // provider/custom entry lands at the end. A later manual drag overwrites this
    // via `set_node_order`/`set_group_order`. Best-effort; never fails generation.
    if snapshot_orders(state, profile_id, &built.yaml)
        .await
        .is_err()
    {
        tracing::warn!(profile = %profile_id, "failed to snapshot node/group order");
    }
    Ok(())
}

/// Persist the output's `proxies`/`proxy-groups` name order into
/// `profiles.node_order`/`group_order`. Empty sequences store NULL.
async fn snapshot_orders(state: &AppState, profile_id: &str, yaml: &str) -> ApiResult<()> {
    let Ok(root) = crate::yaml::parse_limited(yaml) else {
        return Ok(());
    };
    let node_order = order_json(&root, "proxies");
    let group_order = order_json(&root, "proxy-groups");
    sqlx::query("UPDATE profiles SET node_order = ?, group_order = ? WHERE id = ?")
        .bind(&node_order)
        .bind(&group_order)
        .bind(profile_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Extract the ordered `name` values of a top-level sequence (`proxies` or
/// `proxy-groups`) and serialize them as a JSON array, or `None` (→ SQL NULL)
/// when there are none.
fn order_json(root: &serde_yaml::Value, key: &str) -> Option<String> {
    let names: Vec<&str> = match root.get(key) {
        Some(serde_yaml::Value::Sequence(items)) => items
            .iter()
            .filter_map(|item| item.get("name").and_then(|v| v.as_str()))
            .collect(),
        _ => Vec::new(),
    };
    if names.is_empty() {
        None
    } else {
        serde_json::to_string(&names).ok()
    }
}

async fn update_last_fetch(state: &AppState, profile_id: &str, status: &str) -> ApiResult<()> {
    sqlx::query("UPDATE profiles SET last_fetch_at = ?, last_fetch_status = ? WHERE id = ?")
        .bind(now())
        .bind(status)
        .bind(profile_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Re-stitch the cached output to reflect the current saved node/group order and
/// ruleset, **without** re-fetching the provider — so a drag-reorder (or rule
/// edit) is served by the public link immediately, not only after the next full
/// generate. Reordering only permutes entries already in the cached output, and
/// the rules block is fully user-defined (provider-independent), so this is
/// equivalent to a regenerate for these operations. No-op when nothing has been
/// generated yet (the order then applies on the first generate).
pub async fn resync_cache(state: &AppState, profile_id: &str) -> ApiResult<()> {
    let Some(cache) = load_cache(state, profile_id).await? else {
        return Ok(());
    };
    let Ok(serde_yaml::Value::Mapping(mut root)) = crate::yaml::parse_limited(&cache.output_yaml)
    else {
        return Ok(());
    };

    // Reorder proxies / proxy-groups by the saved manual orders.
    let node_order = load_order_col(state, profile_id, "node_order").await?;
    let group_order = load_order_col(state, profile_id, "group_order").await?;
    reorder_seq(&mut root, "proxies", &node_order);
    reorder_seq(&mut root, "proxy-groups", &group_order);

    // Replace the rules block with the current ruleset (order is significant);
    // mirrors the converter (skip blank/comment lines, keep order).
    let rules =
        sqlx::query_scalar::<_, String>("SELECT content FROM rulesets WHERE profile_id = ?")
            .bind(profile_id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_default();
    let rule_values: Vec<serde_yaml::Value> = rules
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(serde_yaml::Value::from)
        .collect();
    root.insert(
        serde_yaml::Value::from("rules"),
        serde_yaml::Value::Sequence(rule_values),
    );

    let Ok(new_yaml) = serde_yaml::to_string(&serde_yaml::Value::Mapping(root)) else {
        return Ok(());
    };
    if new_yaml == cache.output_yaml {
        return Ok(());
    }

    // Patch the cached output in place; keep `generated_at` so the provider
    // refetch cadence is unchanged (content is still the last fetch, reordered).
    sqlx::query(
        "UPDATE generated_cache SET output_yaml = ?, content_hash = ? WHERE profile_id = ?",
    )
    .bind(&new_yaml)
    .bind(hash_inputs("", &new_yaml))
    .bind(profile_id)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Reorder a top-level `proxies`/`proxy-groups` sequence in place by name.
fn reorder_seq(root: &mut serde_yaml::Mapping, key: &str, order: &[String]) {
    if let Some(serde_yaml::Value::Sequence(seq)) = root.get_mut(key) {
        converter::reorder_by_name(seq, |item| item.get("name").and_then(|v| v.as_str()), order);
    }
}

/// Read a profile's `node_order`/`group_order` JSON array (NULL/garbage → empty).
async fn load_order_col(
    state: &AppState,
    profile_id: &str,
    column: &str,
) -> ApiResult<Vec<String>> {
    let sql = match column {
        "node_order" => "SELECT node_order FROM profiles WHERE id = ?",
        _ => "SELECT group_order FROM profiles WHERE id = ?",
    };
    Ok(sqlx::query_scalar::<_, Option<String>>(sql)
        .bind(profile_id)
        .fetch_optional(&state.db)
        .await?
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default())
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn is_fresh(generated_at: &str, ttl: Duration) -> bool {
    let Ok(generated) = chrono::DateTime::parse_from_rfc3339(generated_at) else {
        return false;
    };
    let age = chrono::Utc::now().signed_duration_since(generated.with_timezone(&chrono::Utc));
    age.to_std().map(|a| a < ttl).unwrap_or(false)
}

fn hash_inputs(provider_body: &str, output_yaml: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(provider_body.as_bytes());
    hasher.update([0u8]);
    hasher.update(output_yaml.as_bytes());
    hex(hasher.finalize().as_slice())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn yaml_body(yaml: String) -> Response {
    ([(header::CONTENT_TYPE, "text/yaml; charset=utf-8")], yaml).into_response()
}

fn public_response(profile_name: &str, served: Served) -> Response {
    let filename = sanitize_filename(profile_name);
    let mut headers = vec![
        (header::CONTENT_TYPE, "text/yaml; charset=utf-8".to_string()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}.yaml\""),
        ),
        (
            header::HeaderName::from_static("profile-update-interval"),
            UPDATE_INTERVAL_HOURS.to_string(),
        ),
    ];
    if let Some(userinfo) = served.userinfo {
        headers.push((
            header::HeaderName::from_static("subscription-userinfo"),
            userinfo,
        ));
    }
    (build_header_map(headers), served.yaml).into_response()
}

fn build_header_map(pairs: Vec<(header::HeaderName, String)>) -> header::HeaderMap {
    let mut map = header::HeaderMap::new();
    for (name, value) in pairs {
        if let Ok(v) = header::HeaderValue::from_str(&value) {
            map.insert(name, v);
        }
    }
    map
}

/// Keep filename-safe characters only, so the value can't break the
/// `Content-Disposition` header or the client's file handling.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "subscription".to_string()
    } else {
        trimmed.to_string()
    }
}
