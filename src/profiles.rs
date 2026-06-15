//! Profile management: profile CRUD plus rules, custom nodes, and custom
//! groups. Conversion (generate/preview/public endpoint) is implemented
//! separately. Contracts follow `docs/api-design.md`.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

use crate::app::AppState;
use crate::error::{ApiError, ApiResult};
use crate::mask::mask_url;
use crate::ssrf::{self, SsrfError};
use crate::util::{now, random_token};
use crate::yaml;

const SOURCE_TYPES: [&str; 4] = ["mihomo", "clash", "surge", "loon"];
const GROUP_TYPES: [&str; 5] = ["select", "url-test", "fallback", "load-balance", "relay"];

// ─── DB row types ─────────────────────────────────────────────────────────────

#[derive(FromRow)]
struct ProfileRow {
    id: String,
    name: String,
    source_type: String,
    source_url: String,
    output_type: String,
    token: String,
    enabled: bool,
    last_fetch_at: Option<String>,
    last_fetch_status: Option<String>,
    last_generated_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct NodeRow {
    id: String,
    name: String,
    node_type: String,
    content: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct GroupRow {
    id: String,
    name: String,
    group_type: String,
    members: String,
    options: Option<String>,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(FromRow)]
struct RulesetRow {
    content: String,
    updated_at: String,
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProfileSummary {
    id: String,
    name: String,
    source_type: String,
    source_url_masked: String,
    output_type: String,
    enabled: bool,
    subscription_url: String,
    last_fetch_at: Option<String>,
    last_fetch_status: Option<String>,
    last_generated_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
pub struct ProfileDetail {
    #[serde(flatten)]
    summary: ProfileSummary,
    rules: Option<Ruleset>,
    nodes: Vec<NodeResponse>,
    groups: Vec<GroupResponse>,
}

#[derive(Serialize)]
struct Ruleset {
    content: String,
    updated_at: String,
}

#[derive(Serialize)]
struct NodeResponse {
    id: String,
    name: String,
    node_type: String,
    content: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct GroupResponse {
    id: String,
    name: String,
    group_type: String,
    members: Vec<String>,
    options: Option<JsonValue>,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

fn summary(state: &AppState, row: ProfileRow) -> ProfileSummary {
    ProfileSummary {
        subscription_url: state.subscription_url(&row.token),
        source_url_masked: mask_url(&row.source_url),
        id: row.id,
        name: row.name,
        source_type: row.source_type,
        output_type: row.output_type,
        enabled: row.enabled,
        last_fetch_at: row.last_fetch_at,
        last_fetch_status: row.last_fetch_status,
        last_generated_at: row.last_generated_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn node_response(row: NodeRow) -> NodeResponse {
    NodeResponse {
        id: row.id,
        name: row.name,
        node_type: row.node_type,
        content: row.content,
        enabled: row.enabled,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn group_response(row: GroupRow) -> GroupResponse {
    GroupResponse {
        members: serde_json::from_str(&row.members).unwrap_or_default(),
        options: row.options.and_then(|o| serde_json::from_str(&o).ok()),
        id: row.id,
        name: row.name,
        group_type: row.group_type,
        enabled: row.enabled,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

// ─── Profile CRUD ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProfile {
    name: String,
    source_type: String,
    source_url: String,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
pub struct UpdateProfile {
    name: Option<String>,
    source_type: Option<String>,
    /// Write-only: absent or empty keeps the stored URL unchanged.
    source_url: Option<String>,
    enabled: Option<bool>,
}

/// Validate a provider URL at write time. This is defense in depth and a better
/// error experience — the authoritative SSRF check still runs at fetch time with
/// DNS resolution and IP pinning (`src/fetch.rs`). Static parts are checked here:
/// scheme, embedded credentials, loopback names, and blocked literal IPs.
/// Hostname-only URLs pass (no DNS lookup happens at write time). Messages are
/// generic per error kind so the raw URL is never echoed.
fn validate_source_url(raw: &str) -> ApiResult<()> {
    let url = url::Url::parse(raw)
        .map_err(|_| ApiError::BadRequest("source_url is not a valid URL".into()))?;
    ssrf::validate_url(&url).map_err(|e| {
        let msg = match e {
            SsrfError::Scheme => "source_url must use http or https",
            SsrfError::Host => "source_url is missing a host",
            SsrfError::Credentials => "source_url must not embed credentials",
            SsrfError::BlockedHost | SsrfError::BlockedIp => {
                "source_url points to a disallowed (local/private) address"
            }
        };
        ApiError::BadRequest(msg.into())
    })
}

async fn load_profile_row(state: &AppState, id: &str) -> ApiResult<ProfileRow> {
    sqlx::query_as::<_, ProfileRow>(
        "SELECT p.*, (SELECT generated_at FROM generated_cache WHERE profile_id = p.id) \
         AS last_generated_at FROM profiles p WHERE p.id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)
}

pub async fn list(State(state): State<Arc<AppState>>) -> ApiResult<impl IntoResponse> {
    let rows = sqlx::query_as::<_, ProfileRow>(
        "SELECT p.*, (SELECT generated_at FROM generated_cache WHERE profile_id = p.id) \
         AS last_generated_at FROM profiles p ORDER BY p.created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    let out: Vec<ProfileSummary> = rows.into_iter().map(|r| summary(&state, r)).collect();
    Ok(Json(out))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProfile>,
) -> ApiResult<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    if !SOURCE_TYPES.contains(&body.source_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "source_type must be one of {SOURCE_TYPES:?}"
        )));
    }
    if body.source_url.trim().is_empty() {
        return Err(ApiError::BadRequest("source_url is required".into()));
    }
    validate_source_url(body.source_url.trim())?;

    let id = uuid::Uuid::new_v4().to_string();
    let token = random_token();
    let ts = now();
    let enabled = body.enabled.unwrap_or(true);

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO profiles (id, name, source_type, source_url, token, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(body.name.trim())
    .bind(&body.source_type)
    .bind(body.source_url.trim())
    .bind(&token)
    .bind(enabled)
    .bind(&ts)
    .bind(&ts)
    .execute(&mut *tx)
    .await?;

    // Start each profile with an empty ruleset (1—1, replaced via PUT).
    sqlx::query("INSERT INTO rulesets (id, profile_id, content, updated_at) VALUES (?, ?, '', ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&id)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    let row = load_profile_row(&state, &id).await?;
    Ok((StatusCode::CREATED, Json(detail(&state, row).await?)))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let row = load_profile_row(&state, &id).await?;
    Ok(Json(detail(&state, row).await?))
}

async fn detail(state: &AppState, row: ProfileRow) -> ApiResult<ProfileDetail> {
    let profile_id = row.id.clone();

    let rules = sqlx::query_as::<_, RulesetRow>(
        "SELECT content, updated_at FROM rulesets WHERE profile_id = ?",
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await?
    .map(|r| Ruleset {
        content: r.content,
        updated_at: r.updated_at,
    });

    let nodes = sqlx::query_as::<_, NodeRow>(
        "SELECT id, name, node_type, content, enabled, created_at, updated_at
         FROM custom_nodes WHERE profile_id = ? ORDER BY created_at ASC",
    )
    .bind(&profile_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(node_response)
    .collect();

    let groups = sqlx::query_as::<_, GroupRow>(
        "SELECT id, name, group_type, members, options, enabled, created_at, updated_at
         FROM custom_groups WHERE profile_id = ? ORDER BY created_at ASC",
    )
    .bind(&profile_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(group_response)
    .collect();

    Ok(ProfileDetail {
        summary: summary(state, row),
        rules,
        nodes,
        groups,
    })
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProfile>,
) -> ApiResult<impl IntoResponse> {
    let existing = load_profile_row(&state, &id).await?;

    let name = body.name.unwrap_or(existing.name);
    let source_type = body.source_type.unwrap_or(existing.source_type);
    if !SOURCE_TYPES.contains(&source_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "source_type must be one of {SOURCE_TYPES:?}"
        )));
    }
    // Write-only URL: keep the stored value unless a non-empty one is provided.
    let source_url = match body.source_url {
        Some(u) if !u.trim().is_empty() => {
            let trimmed = u.trim();
            validate_source_url(trimmed)?;
            trimmed.to_string()
        }
        _ => existing.source_url,
    };
    let enabled = body.enabled.unwrap_or(existing.enabled);

    sqlx::query(
        "UPDATE profiles SET name = ?, source_type = ?, source_url = ?, enabled = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&source_type)
    .bind(&source_url)
    .bind(enabled)
    .bind(now())
    .bind(&id)
    .execute(&state.db)
    .await?;

    let row = load_profile_row(&state, &id).await?;
    Ok(Json(detail(&state, row).await?))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let result = sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct TokenResponse {
    token: String,
    subscription_url: String,
}

pub async fn reset_token(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    let token = random_token();
    sqlx::query("UPDATE profiles SET token = ?, updated_at = ? WHERE id = ?")
        .bind(&token)
        .bind(now())
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(Json(TokenResponse {
        subscription_url: state.subscription_url(&token),
        token,
    }))
}

// ─── Rules ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PutRules {
    content: String,
}

pub async fn put_rules(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PutRules>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    sqlx::query("UPDATE rulesets SET content = ?, updated_at = ? WHERE profile_id = ?")
        .bind(&body.content)
        .bind(now())
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Custom nodes ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NodeBody {
    name: String,
    node_type: String,
    content: String,
    enabled: Option<bool>,
}

/// A provider/custom proxy as it appears in the generated output, surfaced for
/// the "node preview" card. Read-only; the frontend marks which names are
/// editable custom nodes by cross-referencing the custom-node list.
#[derive(Serialize)]
struct ProxyPreview {
    name: String,
    #[serde(rename = "type")]
    proxy_type: String,
}

#[derive(Serialize)]
struct ProxiesResponse {
    /// Whether a generated cache exists yet (provider nodes are parsed from it).
    generated: bool,
    generated_at: Option<String>,
    proxies: Vec<ProxyPreview>,
    /// Proxy-groups in the generated output (name + type), for the group
    /// preview and for member suggestions.
    groups: Vec<ProxyPreview>,
}

/// List every proxy in the latest generated output (provider proxies + merged
/// custom nodes). Parsed from `generated_cache.output_yaml`; returns
/// `generated: false` with an empty list when the profile has never been
/// generated.
pub async fn list_proxies(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    let cache = sqlx::query_as::<_, (String, String)>(
        "SELECT output_yaml, generated_at FROM generated_cache WHERE profile_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await?;

    let Some((output_yaml, generated_at)) = cache else {
        return Ok(Json(ProxiesResponse {
            generated: false,
            generated_at: None,
            proxies: Vec::new(),
            groups: Vec::new(),
        }));
    };

    // The output is trusted (we produced it), but parse through the bounded
    // parser anyway and degrade gracefully to an empty list on any surprise.
    let (mut proxies, mut groups) = yaml::parse_limited(&output_yaml)
        .ok()
        .map(|v| {
            (
                extract_previews(&v, "proxies"),
                extract_previews(&v, "proxy-groups"),
            )
        })
        .unwrap_or_default();

    // Reflect a saved manual order even before the profile is regenerated (the
    // cached YAML may still carry the pre-reorder sequence).
    let node_order = load_order(&state, &id, OrderKind::Node).await?;
    let group_order = load_order(&state, &id, OrderKind::Group).await?;
    crate::converter::reorder_by_name(&mut proxies, |p| Some(p.name.as_str()), &node_order);
    crate::converter::reorder_by_name(&mut groups, |g| Some(g.name.as_str()), &group_order);

    Ok(Json(ProxiesResponse {
        generated: true,
        generated_at: Some(generated_at),
        proxies,
        groups,
    }))
}

/// Extract `name` + `type` previews from a top-level sequence (`proxies` or
/// `proxy-groups`) of the generated output.
fn extract_previews(root: &serde_yaml::Value, key: &str) -> Vec<ProxyPreview> {
    match root.get(key) {
        Some(serde_yaml::Value::Sequence(items)) => items
            .iter()
            .filter_map(|item| {
                let name = item.get("name").and_then(|v| v.as_str())?.to_string();
                let proxy_type = item
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Some(ProxyPreview { name, proxy_type })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Which manual-ordering column a request targets. Maps to a fixed column name
/// so the SQL is never built from caller input.
#[derive(Clone, Copy)]
enum OrderKind {
    Node,
    Group,
}

/// Read a profile's persisted manual order (proxy or proxy-group names).
/// Missing/NULL or malformed JSON yields an empty list (= default order).
async fn load_order(state: &AppState, profile_id: &str, kind: OrderKind) -> ApiResult<Vec<String>> {
    let sql = match kind {
        OrderKind::Node => "SELECT node_order FROM profiles WHERE id = ?",
        OrderKind::Group => "SELECT group_order FROM profiles WHERE id = ?",
    };
    Ok(sqlx::query_scalar::<_, Option<String>>(sql)
        .bind(profile_id)
        .fetch_optional(&state.db)
        .await?
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default())
}

#[derive(Deserialize)]
pub struct OrderBody {
    /// Ordered names (provider + custom). Names not present in the profile are
    /// ignored at generation; an empty array clears the manual order.
    order: Vec<String>,
}

/// Bounds to keep a single profile's persisted order small and the request
/// cheap to validate. A profile realistically has well under this many entries.
const MAX_ORDER_ENTRIES: usize = 5_000;
const MAX_ORDER_NAME_LEN: usize = 256;

/// Validate and persist a manual ordering for the given column. Drives both the
/// generated output (applied on next generate) and the preview list.
async fn set_order(
    state: &AppState,
    id: &str,
    kind: OrderKind,
    body: OrderBody,
) -> ApiResult<StatusCode> {
    let _ = load_profile_row(state, id).await?;

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

    // Empty list clears the manual order (back to default); store NULL.
    let stored = if body.order.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&body.order)
                .map_err(|_| ApiError::BadRequest("order could not be serialized".into()))?,
        )
    };

    let sql = match kind {
        OrderKind::Node => "UPDATE profiles SET node_order = ?, updated_at = ? WHERE id = ?",
        OrderKind::Group => "UPDATE profiles SET group_order = ?, updated_at = ? WHERE id = ?",
    };
    sqlx::query(sql)
        .bind(&stored)
        .bind(now())
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `PUT /api/profiles/:id/node-order` — persist a manual proxy ordering.
pub async fn set_node_order(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<OrderBody>,
) -> ApiResult<impl IntoResponse> {
    set_order(&state, &id, OrderKind::Node, body).await
}

/// `PUT /api/profiles/:id/group-order` — persist a manual proxy-group ordering.
pub async fn set_group_order(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<OrderBody>,
) -> ApiResult<impl IntoResponse> {
    set_order(&state, &id, OrderKind::Group, body).await
}

fn validate_node(body: &NodeBody) -> ApiResult<()> {
    if body.name.trim().is_empty() || body.node_type.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "name and node_type are required".into(),
        ));
    }
    // Node content must be a single valid Mihomo proxy mapping.
    yaml::parse_mapping(&body.content)
        .map_err(|_| ApiError::BadRequest("content must be a valid YAML proxy mapping".into()))?;
    Ok(())
}

pub async fn list_nodes(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    let nodes: Vec<NodeResponse> = sqlx::query_as::<_, NodeRow>(
        "SELECT id, name, node_type, content, enabled, created_at, updated_at
         FROM custom_nodes WHERE profile_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(node_response)
    .collect();
    Ok(Json(nodes))
}

pub async fn create_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<NodeBody>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    validate_node(&body)?;
    let node_id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO custom_nodes (id, profile_id, name, node_type, content, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&node_id)
    .bind(&id)
    .bind(body.name.trim())
    .bind(&body.node_type)
    .bind(&body.content)
    .bind(body.enabled.unwrap_or(true))
    .bind(&ts)
    .bind(&ts)
    .execute(&state.db)
    .await?;
    let row = fetch_node(&state, &id, &node_id).await?;
    Ok((StatusCode::CREATED, Json(node_response(row))))
}

pub async fn update_node(
    State(state): State<Arc<AppState>>,
    Path((id, node_id)): Path<(String, String)>,
    Json(body): Json<NodeBody>,
) -> ApiResult<impl IntoResponse> {
    let _ = fetch_node(&state, &id, &node_id).await?;
    validate_node(&body)?;
    sqlx::query(
        "UPDATE custom_nodes SET name = ?, node_type = ?, content = ?, enabled = ?, updated_at = ?
         WHERE id = ? AND profile_id = ?",
    )
    .bind(body.name.trim())
    .bind(&body.node_type)
    .bind(&body.content)
    .bind(body.enabled.unwrap_or(true))
    .bind(now())
    .bind(&node_id)
    .bind(&id)
    .execute(&state.db)
    .await?;
    let row = fetch_node(&state, &id, &node_id).await?;
    Ok(Json(node_response(row)))
}

pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Path((id, node_id)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    let result = sqlx::query("DELETE FROM custom_nodes WHERE id = ? AND profile_id = ?")
        .bind(&node_id)
        .bind(&id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_node(state: &AppState, profile_id: &str, node_id: &str) -> ApiResult<NodeRow> {
    sqlx::query_as::<_, NodeRow>(
        "SELECT id, name, node_type, content, enabled, created_at, updated_at
         FROM custom_nodes WHERE id = ? AND profile_id = ?",
    )
    .bind(node_id)
    .bind(profile_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)
}

// ─── Custom groups ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GroupBody {
    name: String,
    group_type: String,
    members: Vec<String>,
    options: Option<JsonValue>,
    enabled: Option<bool>,
}

fn validate_group(body: &GroupBody) -> ApiResult<()> {
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    if !GROUP_TYPES.contains(&body.group_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "group_type must be one of {GROUP_TYPES:?}"
        )));
    }
    if let Some(options) = &body.options {
        if !options.is_object() {
            return Err(ApiError::BadRequest("options must be a JSON object".into()));
        }
    }
    Ok(())
}

pub async fn list_groups(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    let groups: Vec<GroupResponse> = sqlx::query_as::<_, GroupRow>(
        "SELECT id, name, group_type, members, options, enabled, created_at, updated_at
         FROM custom_groups WHERE profile_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(group_response)
    .collect();
    Ok(Json(groups))
}

pub async fn create_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<GroupBody>,
) -> ApiResult<impl IntoResponse> {
    let _ = load_profile_row(&state, &id).await?;
    validate_group(&body)?;
    let group_id = uuid::Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO custom_groups (id, profile_id, name, group_type, members, options, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&group_id)
    .bind(&id)
    .bind(body.name.trim())
    .bind(&body.group_type)
    .bind(serde_json::to_string(&body.members).unwrap_or_else(|_| "[]".into()))
    .bind(body.options.as_ref().map(|o| o.to_string()))
    .bind(body.enabled.unwrap_or(true))
    .bind(&ts)
    .bind(&ts)
    .execute(&state.db)
    .await?;
    let row = fetch_group(&state, &id, &group_id).await?;
    Ok((StatusCode::CREATED, Json(group_response(row))))
}

pub async fn update_group(
    State(state): State<Arc<AppState>>,
    Path((id, group_id)): Path<(String, String)>,
    Json(body): Json<GroupBody>,
) -> ApiResult<impl IntoResponse> {
    let _ = fetch_group(&state, &id, &group_id).await?;
    validate_group(&body)?;
    sqlx::query(
        "UPDATE custom_groups SET name = ?, group_type = ?, members = ?, options = ?, enabled = ?, updated_at = ?
         WHERE id = ? AND profile_id = ?",
    )
    .bind(body.name.trim())
    .bind(&body.group_type)
    .bind(serde_json::to_string(&body.members).unwrap_or_else(|_| "[]".into()))
    .bind(body.options.as_ref().map(|o| o.to_string()))
    .bind(body.enabled.unwrap_or(true))
    .bind(now())
    .bind(&group_id)
    .bind(&id)
    .execute(&state.db)
    .await?;
    let row = fetch_group(&state, &id, &group_id).await?;
    Ok(Json(group_response(row)))
}

pub async fn delete_group(
    State(state): State<Arc<AppState>>,
    Path((id, group_id)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    let result = sqlx::query("DELETE FROM custom_groups WHERE id = ? AND profile_id = ?")
        .bind(&group_id)
        .bind(&id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn fetch_group(state: &AppState, profile_id: &str, group_id: &str) -> ApiResult<GroupRow> {
    sqlx::query_as::<_, GroupRow>(
        "SELECT id, name, group_type, members, options, enabled, created_at, updated_at
         FROM custom_groups WHERE id = ? AND profile_id = ?",
    )
    .bind(group_id)
    .bind(profile_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)
}
