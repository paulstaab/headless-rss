use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::{delete, get, post, put};

use crate::config::Config;

use super::AppState;

pub(super) fn router(config_for_middleware: Arc<Config>) -> Router<AppState> {
    Router::new()
        .route("/feeds", get(super::get_feeds))
        .route("/feeds", post(super::v1_2_add_feed))
        .route("/feeds/{feed_id}", delete(super::delete_feed))
        .route("/feeds/{feed_id}/move", put(super::v1_2_move_feed))
        .route("/feeds/{feed_id}/rename", put(super::v1_2_rename_feed))
        .route(
            "/feeds/{feed_id}/read",
            put(super::v1_2_mark_feed_items_read),
        )
        .route("/folders", get(super::get_folders))
        .route("/folders", post(super::create_folder))
        .route("/folders/{folder_id}", delete(super::delete_folder))
        .route("/folders/{folder_id}", put(super::rename_folder))
        .route(
            "/folders/{folder_id}/read",
            post(super::mark_folder_items_read),
        )
        .route("/items", get(super::get_items))
        .route("/items/updated", get(super::get_updated_items))
        .route("/items/{item_id}/content", get(super::get_item_content))
        .route("/items/{item_id}/read", post(super::v1_2_mark_item_as_read))
        .route(
            "/items/read/multiple",
            put(super::v1_2_mark_multiple_items_as_read),
        )
        .route(
            "/items/{item_id}/unread",
            put(super::v1_2_mark_item_as_unread),
        )
        .route(
            "/items/unread/multiple",
            put(super::v1_2_mark_multiple_items_as_unread),
        )
        .route(
            "/items/{feed_id}/{guid_hash}/star",
            put(super::v1_2_mark_item_as_starred),
        )
        .route(
            "/items/star/multiple",
            put(super::v1_2_mark_multiple_items_as_starred),
        )
        .route(
            "/items/{feed_id}/{guid_hash}/unstar",
            put(super::v1_2_mark_item_as_unstarred),
        )
        .route(
            "/items/unstar/multiple",
            put(super::v1_2_mark_multiple_items_as_unstarred),
        )
        .route("/items/read", put(super::v1_2_mark_all_items_as_read))
        .route("/version", get(super::get_version))
        .route_layer(middleware::from_fn_with_state(
            config_for_middleware,
            super::require_basic_auth,
        ))
}
