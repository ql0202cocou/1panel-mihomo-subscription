//! 应用设置:读取设置,以及重置全局公共路径前缀。

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;

use crate::app::AppState;
use crate::error::ApiResult;
use crate::util::{now, random_path_prefix};

#[derive(Serialize)]
pub struct SettingsResponse {
    public_path_prefix: String,
}

pub async fn get(State(state): State<Arc<AppState>>) -> ApiResult<impl IntoResponse> {
    Ok(Json(SettingsResponse {
        public_path_prefix: state.current_prefix(),
    }))
}

/// 重置全局公共路径前缀。所有 profile 的托管链接一次性改变,使全部旧链接失效
/// (见 `docs/security-design.md`)。
pub async fn reset_public_path(State(state): State<Arc<AppState>>) -> ApiResult<impl IntoResponse> {
    let prefix = random_path_prefix();
    sqlx::query("UPDATE app_settings SET public_path_prefix = ?, updated_at = ? WHERE id = 1")
        .bind(&prefix)
        .bind(now())
        .execute(&state.db)
        .await?;
    state.set_prefix(prefix.clone());
    Ok(Json(SettingsResponse {
        public_path_prefix: prefix,
    }))
}
