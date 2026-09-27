//! problem+json error contract (docs/api.md preamble): every failure renders
//! `{ "error": { "code", "message", "request_id" } }` with the HTTP code from
//! the standard table. Ledger gRPC statuses are translated here so internal
//! details never leak (docs/security-controls.md §28), while domain messages
//! (safe by construction in the ledger's `to_status`) are preserved so the
//! client can show the reason.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 400 — validation failure (field-level message).
    #[error("{0}")]
    Validation(String),
    /// 401 — missing caller / auth (gateway will own real auth).
    #[error("missing or invalid caller identity")]
    Unauthorized,
    /// 404 — unknown resource.
    #[error("{0}")]
    NotFound(String),
    /// 409 — idempotency replay with a different request, or duplicate link.
    #[error("{0}")]
    Conflict(String),
    /// 422 — the ledger refused the money movement (insufficient funds, hold
    /// state machine).
    #[error("{0}")]
    Unprocessable(String),
    /// 503 — ledger/storage unavailable.
    #[error("{0}")]
    Unavailable(String),
    /// 500 — anything unexpected. Message is generic; details go to logs.
    #[error("internal error")]
    Internal,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::Validation(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ApiError::Validation(_) => "validation_failed",
            ApiError::Unauthorized => "unauthorized",
            ApiError::NotFound(_) => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::Unprocessable(_) => "ledger_rejected",
            ApiError::Unavailable(_) => "unavailable",
            ApiError::Internal => "internal_error",
        }
    }
}

/// Translate a ledger gRPC status into the REST error contract. The ledger's
/// own `to_status` already guarantees safe messages; code mapping follows the
/// docs/api.md table.
impl From<tonic::Status> for ApiError {
    fn from(status: tonic::Status) -> Self {
        match status.code() {
            tonic::Code::InvalidArgument => ApiError::Validation(status.message().to_string()),
            tonic::Code::NotFound => ApiError::NotFound(status.message().to_string()),
            tonic::Code::FailedPrecondition => {
                ApiError::Unprocessable(status.message().to_string())
            }
            tonic::Code::AlreadyExists => ApiError::Conflict(status.message().to_string()),
            tonic::Code::PermissionDenied | tonic::Code::Unauthenticated => ApiError::Unauthorized,
            tonic::Code::Unavailable | tonic::Code::DeadlineExceeded => {
                ApiError::Unavailable("ledger is unreachable — try again".to_string())
            }
            _ => ApiError::Internal,
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!(error = %err, "core database error");
        // Never surface SQL details to the client.
        ApiError::Unavailable("storage unavailable — try again".to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = json!({
            "error": {
                "code": self.code(),
                "message": match &self {
                    // 500 stays generic even though thiserror flattens the
                    // Display into a unit message already.
                    ApiError::Internal => "internal error".to_string(),
                    other => other.to_string(),
                },
                "request_id": uuid::Uuid::new_v4().to_string(),
            }
        });
        (status, Json(body)).into_response()
    }
}
