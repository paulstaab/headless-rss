use std::sync::Arc;

use axum::Router;
use axum::middleware;
use axum::routing::{delete, get, post, put};

use crate::config::Config;

use super::AppState;

pub(super) fn router(config_for_middleware: Arc<Config>) -> Router<AppState> {
    Router::new()
        .route("/feeds", get(super::feeds::get_feeds))
        .route("/feeds", post(super::feeds::v1_3_add_feed))
        .route("/feeds/{feed_id}", delete(super::feeds::delete_feed))
        .route("/feeds/{feed_id}/move", post(super::feeds::v1_3_move_feed))
        .route(
            "/feeds/{feed_id}/rename",
            post(super::feeds::v1_3_rename_feed),
        )
        .route(
            "/feeds/{feed_id}/read",
            post(super::feeds::v1_3_mark_feed_items_read),
        )
        .route("/folders", get(super::folders::get_folders))
        .route("/folders", post(super::folders::create_folder))
        .route(
            "/folders/{folder_id}",
            delete(super::folders::delete_folder),
        )
        .route("/folders/{folder_id}", put(super::folders::rename_folder))
        .route(
            "/folders/{folder_id}/read",
            post(super::folders::mark_folder_items_read),
        )
        .route("/items", get(super::items::get_items))
        .route("/items/updated", get(super::items::get_updated_items))
        .route(
            "/items/{item_id}/content",
            get(super::items::get_item_content),
        )
        .route(
            "/items/{item_id}/read",
            post(super::items::v1_3_mark_item_as_read),
        )
        .route(
            "/items/read/multiple",
            post(super::items::v1_3_mark_multiple_items_as_read),
        )
        .route(
            "/items/{item_id}/unread",
            post(super::items::v1_3_mark_item_as_unread),
        )
        .route(
            "/items/unread/multiple",
            post(super::items::v1_3_mark_multiple_items_as_unread),
        )
        .route(
            "/items/star/multiple",
            post(super::items::v1_3_mark_multiple_items_as_starred),
        )
        .route(
            "/items/{item_id}/star",
            post(super::items::v1_3_mark_item_as_starred),
        )
        .route(
            "/items/{item_id}/unstar",
            post(super::items::v1_3_mark_item_as_unstarred),
        )
        .route(
            "/items/unstar/multiple",
            post(super::items::v1_3_mark_multiple_items_as_unstarred),
        )
        .route(
            "/items/read",
            post(super::items::v1_3_mark_all_items_as_read),
        )
        .route("/version", get(super::get_version))
        .route_layer(middleware::from_fn_with_state(
            config_for_middleware,
            super::auth::require_basic_auth,
        ))
}
