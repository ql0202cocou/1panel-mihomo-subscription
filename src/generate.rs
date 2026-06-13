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

    Ok(converter::convert(ConvertInput {
        provider_yaml,
        rules: &rules,
        nodes,
        groups,
    }))
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
    Ok(())
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
