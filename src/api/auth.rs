//! Authentication middleware for protected Nextcloud-compatible API routes.

use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::config::Config;

/// Enforces optional basic authentication when credentials are configured.
pub(super) async fn require_basic_auth(
    State(config): State<Arc<Config>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !config.auth_enabled() {
        return next.run(request).await;
    }

    let Some(auth_header) = request.headers().get(header::AUTHORIZATION) else {
        return unauthorized("Not authenticated");
    };

    let Ok(auth_header) = auth_header.to_str() else {
        return unauthorized("Invalid authentication credentials");
    };

    if !auth_header.starts_with("Basic ") {
        return unauthorized("Invalid authentication credentials");
    }

    let encoded = &auth_header[6..];
    let Ok(decoded_bytes) = BASE64.decode(encoded) else {
        return unauthorized("Invalid authentication credentials");
    };

    let Ok(decoded) = String::from_utf8(decoded_bytes) else {
        return unauthorized("Invalid authentication credentials");
    };

    let Some((username, password)) = decoded.split_once(':') else {
        return unauthorized("Invalid authentication credentials");
    };

    let expected_username = config.username.as_deref().unwrap_or_default();
    let expected_password = config.password.as_deref().unwrap_or_default();

    if username != expected_username || password != expected_password {
        return unauthorized("Invalid authentication credentials");
    }

    next.run(request).await
}

fn unauthorized(message: &'static str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Basic")],
        Json(serde_json::json!({ "detail": message })),
    )
        .into_response()
}
