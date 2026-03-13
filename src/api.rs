//! HTTP API composition for the Rust Nextcloud News compatible service.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;

use crate::config::Config;

mod auth;
mod errors;
mod feeds;
mod folders;
mod items;

mod v1_2;
mod v1_3;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub feed_http_client: reqwest::Client,
    pub article_http_client: reqwest::Client,
}

#[derive(Serialize)]
struct StatusOut {
    status: &'static str,
}

#[derive(Serialize)]
struct VersionOut {
    version: String,
}

/// Builds the full Axum router for both supported Nextcloud News API versions.
pub fn app(state: AppState) -> Router {
    let protected_v1_2 = v1_2::router(state.config.clone());
    let protected_v1_3 = v1_3::router(state.config.clone());

    Router::new()
        .route("/status", get(status))
        .nest("/index.php/apps/news/api/v1-2", protected_v1_2)
        .nest("/index.php/apps/news/api/v1-3", protected_v1_3)
        .layer(axum::middleware::from_fn(log_user_interaction))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

/// Logs each incoming API request together with the response status and duration.
async fn log_user_interaction(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let started_at = Instant::now();
    let response = next.run(request).await;
    let duration_ms = started_at.elapsed().as_millis() as u64;

    tracing::info!(
        method = %method,
        uri = %uri,
        status = response.status().as_u16(),
        duration_ms,
        "user interaction handled"
    );

    response
}

/// Returns the unauthenticated service health check response.
async fn status() -> Json<StatusOut> {
    Json(StatusOut { status: "ok" })
}

/// Returns the configured application version for Nextcloud News compatibility.
async fn get_version(State(state): State<AppState>) -> Json<VersionOut> {
    Json(VersionOut {
        version: state.config.version.clone(),
    })
}

#[cfg(test)]
mod tests;
