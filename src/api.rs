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

#[derive(Clone, Copy)]
enum ApiVersion {
    V1_2,
    V1_3,
}

/// Builds the full Axum router for both supported Nextcloud News API versions.
pub fn app(state: AppState) -> Router {
    let protected_v1_2 = protected_router(state.config.clone(), ApiVersion::V1_2);
    let protected_v1_3 = protected_router(state.config.clone(), ApiVersion::V1_3);

    Router::new()
        .route("/status", get(status))
        .nest("/index.php/apps/news/api/v1-2", protected_v1_2)
        .nest("/index.php/apps/news/api/v1-3", protected_v1_3)
        .layer(axum::middleware::from_fn(log_user_interaction))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

/// Builds one protected Nextcloud-compatible router, varying only the version-specific routes.
fn protected_router(config_for_middleware: Arc<Config>, version: ApiVersion) -> Router<AppState> {
    let router = Router::new()
        .route("/feeds", get(feeds::get_feeds))
        .route("/feeds", axum::routing::post(feeds::add_feed))
        .route(
            "/feeds/{feed_id}",
            axum::routing::delete(feeds::delete_feed),
        )
        .route("/folders", get(folders::get_folders))
        .route("/folders", axum::routing::post(folders::create_folder))
        .route(
            "/folders/{folder_id}",
            axum::routing::delete(folders::delete_folder),
        )
        .route(
            "/folders/{folder_id}",
            axum::routing::put(folders::rename_folder),
        )
        .route(
            "/folders/{folder_id}/read",
            axum::routing::post(folders::mark_folder_items_read),
        )
        .route("/items", get(items::get_items))
        .route("/items/updated", get(items::get_updated_items))
        .route("/items/{item_id}/content", get(items::get_item_content))
        .route("/version", get(get_version));

    let router = match version {
        ApiVersion::V1_2 => router
            .route(
                "/feeds/{feed_id}/move",
                axum::routing::put(feeds::move_feed),
            )
            .route(
                "/feeds/{feed_id}/rename",
                axum::routing::put(feeds::rename_feed),
            )
            .route(
                "/feeds/{feed_id}/read",
                axum::routing::put(feeds::mark_feed_items_read),
            )
            .route(
                "/items/{item_id}/read",
                axum::routing::post(items::v1_2_mark_item_as_read),
            )
            .route(
                "/items/read/multiple",
                axum::routing::put(items::v1_2_mark_multiple_items_as_read),
            )
            .route(
                "/items/{item_id}/unread",
                axum::routing::put(items::v1_2_mark_item_as_unread),
            )
            .route(
                "/items/unread/multiple",
                axum::routing::put(items::v1_2_mark_multiple_items_as_unread),
            )
            .route(
                "/items/{feed_id}/{guid_hash}/star",
                axum::routing::put(items::v1_2_mark_item_as_starred),
            )
            .route(
                "/items/star/multiple",
                axum::routing::put(items::v1_2_mark_multiple_items_as_starred),
            )
            .route(
                "/items/{feed_id}/{guid_hash}/unstar",
                axum::routing::put(items::v1_2_mark_item_as_unstarred),
            )
            .route(
                "/items/unstar/multiple",
                axum::routing::put(items::v1_2_mark_multiple_items_as_unstarred),
            )
            .route(
                "/items/read",
                axum::routing::put(items::mark_all_items_as_read_v1_2),
            ),
        ApiVersion::V1_3 => router
            .route(
                "/feeds/{feed_id}/move",
                axum::routing::post(feeds::move_feed),
            )
            .route(
                "/feeds/{feed_id}/rename",
                axum::routing::post(feeds::rename_feed),
            )
            .route(
                "/feeds/{feed_id}/read",
                axum::routing::post(feeds::mark_feed_items_read),
            )
            .route(
                "/items/{item_id}/read",
                axum::routing::post(items::v1_3_mark_item_as_read),
            )
            .route(
                "/items/read/multiple",
                axum::routing::post(items::v1_3_mark_multiple_items_as_read),
            )
            .route(
                "/items/{item_id}/unread",
                axum::routing::post(items::v1_3_mark_item_as_unread),
            )
            .route(
                "/items/unread/multiple",
                axum::routing::post(items::v1_3_mark_multiple_items_as_unread),
            )
            .route(
                "/items/star/multiple",
                axum::routing::post(items::v1_3_mark_multiple_items_as_starred),
            )
            .route(
                "/items/{item_id}/star",
                axum::routing::post(items::v1_3_mark_item_as_starred),
            )
            .route(
                "/items/{item_id}/unstar",
                axum::routing::post(items::v1_3_mark_item_as_unstarred),
            )
            .route(
                "/items/unstar/multiple",
                axum::routing::post(items::v1_3_mark_multiple_items_as_unstarred),
            )
            .route(
                "/items/read",
                axum::routing::post(items::mark_all_items_as_read_v1_3),
            ),
    };

    router.route_layer(axum::middleware::from_fn_with_state(
        config_for_middleware,
        auth::require_basic_auth,
    ))
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
        "request"
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
