//! Shared API result types and HTTP error helpers.

use axum::Json;
use axum::http::StatusCode;

/// Standard error shape used by API handlers.
pub(super) type ApiError = (StatusCode, Json<serde_json::Value>);
/// Standard result type used by API handlers.
pub(super) type ApiResult<T> = Result<T, ApiError>;

/// Returns a conflict error when creating a duplicate feed.
pub(super) fn feed_already_exists() -> ApiError {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "detail": "Feed already exists" })),
    )
}

/// Returns an unprocessable-entity error for invalid feed content.
pub(super) fn feed_parse_error<E: std::fmt::Display>(error: E) -> ApiError {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({ "detail": error.to_string() })),
    )
}

/// Returns a bad-request error for SSRF validation failures.
pub(super) fn ssrf_error<E: std::fmt::Display>(error: E) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "detail": error.to_string() })),
    )
}

/// Returns a bad-request error with a caller-provided detail message.
pub(super) fn bad_request_error<E: std::fmt::Display>(error: E) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "detail": error.to_string() })),
    )
}

/// Returns a not-found error for a missing feed identifier.
pub(super) fn feed_not_found_with_id(feed_id: i64) -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "detail": format!("Feed {feed_id} not found") })),
    )
}

/// Returns a not-found error for a missing folder resource.
pub(super) fn folder_not_found() -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "detail": "Folder not found" })),
    )
}

/// Returns a validation error for a missing destination folder id.
pub(super) fn folder_not_found_with_id(folder_id: i64) -> ApiError {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({
            "detail": format!("Folder with ID {folder_id} does not exist"),
        })),
    )
}

/// Returns a conflict error for duplicate folder names.
pub(super) fn folder_already_exists() -> ApiError {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "detail": "Folder already exists" })),
    )
}

/// Returns a validation error for empty folder names.
pub(super) fn folder_name_invalid() -> ApiError {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({ "detail": "Folder name is invalid" })),
    )
}

/// Maps SQLx errors to a standard internal-server response.
pub(super) fn internal_error(error: sqlx::Error) -> ApiError {
    let _ = error;
    tracing::error!("database operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "detail": "Internal server error" })),
    )
}

/// Maps anyhow errors to a standard internal-server response.
pub(super) fn internal_anyhow_error(error: anyhow::Error) -> ApiError {
    let _ = error;
    tracing::error!("application operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "detail": "Internal server error" })),
    )
}

/// Returns a not-found error for a missing item resource.
pub(super) fn item_not_found() -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "detail": "Item not found" })),
    )
}
