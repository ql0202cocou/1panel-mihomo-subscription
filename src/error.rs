//! API error type mapping to the documented error envelope and status codes
//! (see `docs/api-design.md`).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    /// Itemized validation failure (e.g. generate-time rule checks).
    Validation(Vec<String>),
    NotFound,
    Conflict(String),
    /// Upstream provider fetch/parse failure; carries a safe status label.
    Upstream(String),
    Internal,
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, String, Option<Vec<String>>) {
        match self {
            ApiError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", msg.clone(), None)
            }
            ApiError::Validation(details) => (
                StatusCode::BAD_REQUEST,
                "validation_failed",
                "Validation failed".to_string(),
                Some(details.clone()),
            ),
            ApiError::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Not found".to_string(),
                None,
            ),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone(), None),
            ApiError::Upstream(label) => (
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                format!("Provider fetch failed: {label}"),
                None,
            ),
            ApiError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Internal server error".to_string(),
                None,
            ),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Vec<String>>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = self.parts();
        let body = ErrorBody {
            error: ErrorDetail {
                code,
                message,
                details,
            },
        };
        (status, Json(body)).into_response()
    }
}

/// Map a SQLx error to an `ApiError`, surfacing UNIQUE violations as `409` and
/// everything else as a masked `500` (the raw error is logged, not returned).
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        if is_unique_violation(&err) {
            return ApiError::Conflict("A resource with the same name already exists".to_string());
        }
        // Safe to log: `sqlx::Error`'s Display carries the driver message
        // (e.g. constraint/column names), never the bound parameter values, so a
        // provider URL or token cannot leak here. The masked `500` is returned to
        // the client; the detail stays server-side. Do not switch this to logging
        // the query + parameters.
        tracing::error!("database error: {err}");
        ApiError::Internal
    }
}

fn is_unique_violation(err: &sqlx::Error) -> bool {
    if let sqlx::Error::Database(db) = err {
        // SQLite: 2067 = SQLITE_CONSTRAINT_UNIQUE, 1555 = PRIMARY KEY.
        return matches!(db.code().as_deref(), Some("2067") | Some("1555"));
    }
    false
}

pub type ApiResult<T> = Result<T, ApiError>;
