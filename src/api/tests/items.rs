use axum::body::Body;
use axum::http::Request;
use serde_json::Value;
use tower::ServiceExt;

use crate::api::app;

use super::support::{setup_pool, state};

#[tokio::test]
async fn items_endpoint_returns_items_with_body() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?batchSize=10&offset=0&type=0&id=10&getRead=true&oldestFirst=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["items"][0]["body"], "summary content");
}

#[tokio::test]
async fn items_endpoint_type_folder_filters_by_folder_id() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (20, 'https://example.com/tech-rss', 'Tech Feed', NULL, 123, NULL, 2, 0, 'https://example.com/tech', 0, 0, NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (200, 'Folder Item', 'content', 'Author', NULL, NULL, NULL, 20, NULL, 'guid-200', 'guid-hash-200', 300, NULL, NULL, 200, 0, 0, 1, 200, 'https://example.com/folder-item', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=1&id=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 200);
}

#[tokio::test]
async fn items_endpoint_type_starred_returns_only_starred_items() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET starred = 1 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Unstarred Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 201, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/unstarred', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=2&id=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 100);
    assert_eq!(items[0]["starred"], true);
}

#[tokio::test]
async fn items_endpoint_get_read_false_returns_only_unread() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET unread = 0 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Unread Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 201, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/unread', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=3&id=0&getRead=false")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 101);
    assert_eq!(items[0]["unread"], true);
}

#[tokio::test]
async fn items_endpoint_oldest_first_true_sorts_ascending() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Second Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 201, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/second', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=3&id=0&oldestFirst=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], 100);
    assert_eq!(items[1]["id"], 101);
}

#[tokio::test]
async fn items_endpoint_batch_size_limits_results() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Second Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 201, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/second', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=3&id=0&batchSize=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 101);
}

#[tokio::test]
async fn items_endpoint_offset_filters_as_newest_item_id() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Second Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 201, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/second', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=3&id=0&offset=100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 100);
}

#[tokio::test]
async fn updated_items_feed_selection_honors_last_modified_threshold() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Older Feed Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 150, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/older-feed-item', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items/updated?lastModified=180&type=0&id=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 100);
}

#[tokio::test]
async fn updated_items_folder_selection_honors_last_modified_threshold() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (20, 'https://example.com/tech-rss', 'Tech Feed', NULL, 123, NULL, 2, 0, 'https://example.com/tech', 0, 0, NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (200, 'Recent Folder Item', 'content', 'Author', NULL, NULL, NULL, 20, NULL, 'guid-200', 'guid-hash-200', 350, NULL, NULL, 200, 0, 0, 1, 200, 'https://example.com/recent-folder-item', NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (201, 'Older Folder Item', 'content', 'Author', NULL, NULL, NULL, 20, NULL, 'guid-201', 'guid-hash-201', 100, NULL, NULL, 201, 0, 0, 1, 201, 'https://example.com/older-folder-item', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items/updated?lastModified=200&type=1&id=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 200);
}

#[tokio::test]
async fn updated_items_starred_selection_honors_last_modified_threshold() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET starred = 1, last_modified = 400 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Old Starred Item', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 150, NULL, NULL, 101, 0, 1, 1, 101, 'https://example.com/old-starred-item', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items/updated?lastModified=300&type=2&id=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], 100);
    assert_eq!(items[0]["starred"], true);
}

#[tokio::test]
async fn updated_items_all_selection_honors_last_modified_and_desc_order() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET last_modified = 250, unread = 0 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (101, 'Recent Item 1', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-101', 'guid-hash-101', 300, NULL, NULL, 101, 0, 0, 1, 101, 'https://example.com/recent-1', NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (102, 'Recent Item 2', 'content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-102', 'guid-hash-102', 320, NULL, NULL, 102, 0, 0, 0, 102, 'https://example.com/recent-2', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items/updated?lastModified=240&type=3&id=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let items = parsed["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0]["id"], 102);
    assert_eq!(items[1]["id"], 101);
    assert_eq!(items[2]["id"], 100);
}

#[tokio::test]
async fn item_content_returns_404_when_missing() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items/999999/content")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn v1_2_star_by_guid_hash_sets_starred() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/10/guid-hash-1/star")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn v1_3_read_multiple_accepts_item_ids_payload() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/read/multiple")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"itemIds":[100]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn get_items_invalid_selection_type_returns_400_with_detail() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/items?type=99&id=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Invalid item selection type");
}

#[tokio::test]
async fn v1_2_mark_item_as_read_missing_returns_404_with_detail() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-2/items/9999/read")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Item not found");
}

#[tokio::test]
async fn v1_2_mark_item_as_read_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-2/items/100/read")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_2_mark_item_as_unread_updates_last_modified() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET unread = 0, last_modified = 200 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/100/unread")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 1);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_2_mark_item_as_unstarred_updates_last_modified() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET starred = 1, last_modified = 200 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/10/guid-hash-1/unstar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (starred, last_modified): (i64, i64) =
        sqlx::query_as("SELECT starred, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(starred, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_item_as_read_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/100/read")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_item_as_unread_updates_last_modified() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET unread = 0, last_modified = 200 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/100/unread")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 1);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_item_as_starred_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/100/star")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (starred, last_modified): (i64, i64) =
        sqlx::query_as("SELECT starred, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(starred, 1);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_item_as_unstarred_updates_last_modified() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET starred = 1, last_modified = 200 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/100/unstar")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (starred, last_modified): (i64, i64) =
        sqlx::query_as("SELECT starred, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(starred, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_item_as_starred_missing_returns_404_with_detail() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/9999/star")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Item not found");
}

#[tokio::test]
async fn v1_2_mark_multiple_items_as_read_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/read/multiple")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"items":[100]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_2_mark_multiple_guid_items_as_starred_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/star/multiple")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"items":[{"feedId":10,"guidHash":"guid-hash-1"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (starred, last_modified): (i64, i64) =
        sqlx::query_as("SELECT starred, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(starred, 1);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_multiple_items_as_unread_updates_last_modified() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET unread = 0, last_modified = 200 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/unread/multiple")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"itemIds":[100]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 1);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_multiple_items_as_unstarred_updates_last_modified() {
    let pool = setup_pool().await;
    sqlx::query("UPDATE article SET starred = 1, last_modified = 200 WHERE id = 100")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/unstar/multiple")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"itemIds":[100]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (starred, last_modified): (i64, i64) =
        sqlx::query_as("SELECT starred, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(starred, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_2_mark_all_items_as_read_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":100}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_3_mark_all_items_as_read_updates_last_modified() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/items/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":100}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let (unread, last_modified): (i64, i64) =
        sqlx::query_as("SELECT unread, last_modified FROM article WHERE id = 100")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unread, 0);
    assert!(last_modified > 200);
}

#[tokio::test]
async fn v1_2_mark_item_as_starred_missing_guid_returns_404_with_detail() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/items/10/missing-guid/star")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Item not found");
}
