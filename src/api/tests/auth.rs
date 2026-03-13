use axum::body::Body;
use axum::http::Request;
use base64::Engine;
use serde_json::Value;
use tower::ServiceExt;

use crate::api::app;

use super::support::{setup_pool, state_with_auth};

#[tokio::test]
async fn protected_endpoints_require_auth_when_configured() {
    let response = app(state_with_auth(setup_pool().await, "user", "pass"))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-2/folders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn protected_endpoints_reject_invalid_credentials() {
    let wrong = base64::engine::general_purpose::STANDARD.encode("wronguser:wrongpass");

    let response = app(state_with_auth(setup_pool().await, "testuser", "testpass"))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-2/feeds")
                .header("authorization", format!("Basic {wrong}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Invalid authentication credentials");
}

#[tokio::test]
async fn protected_endpoints_accept_valid_credentials() {
    let ok = base64::engine::general_purpose::STANDARD.encode("testuser:testpass");

    let response = app(state_with_auth(setup_pool().await, "testuser", "testpass"))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-2/feeds")
                .header("authorization", format!("Basic {ok}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}
