use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

use crate::api::app;

use super::support::{setup_pool, state};

#[tokio::test]
async fn status_endpoint_returns_ok() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}
