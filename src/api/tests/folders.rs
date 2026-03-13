use axum::body::Body;
use axum::http::Request;
use serde_json::Value;
use tower::ServiceExt;

use crate::api::app;

use super::support::{setup_pool, state};

#[tokio::test]
async fn create_folder_returns_new_folder() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/folders")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Media"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn create_folder_duplicate_returns_409() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/folders")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Tech"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 409);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Folder already exists");
}

#[tokio::test]
async fn create_folder_invalid_name_returns_422() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/folders")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 422);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Folder name is invalid");
}

#[tokio::test]
async fn delete_nonexistent_folder_returns_404() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/index.php/apps/news/api/v1-3/folders/9999")
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
    assert_eq!(parsed["detail"], "Folder not found");
}

#[tokio::test]
async fn rename_folder_duplicate_returns_409() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO folder (id, name, is_root) VALUES (3, 'News', 0)")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-3/folders/2")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"News"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 409);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Folder already exists");
}

#[tokio::test]
async fn rename_folder_invalid_name_returns_422() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-3/folders/2")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 422);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Folder name is invalid");
}

#[tokio::test]
async fn folder_read_marks_folder_items_as_read() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (20, 'https://example.com/tech', 'Tech Feed', NULL, 123, NULL, 2, 0, 'https://example.com/tech', 0, 0, NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (200, 'Folder Article', 'content', 'Author', NULL, NULL, NULL, 20, NULL, 'guid-200', 'guid-hash-200', 200, NULL, NULL, 100, 0, 0, 1, 100, 'https://example.com/tech/1', 'summary')")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/folders/2/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":200}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let unread: i64 = sqlx::query_scalar("SELECT unread FROM article WHERE id = 200")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unread, 0);
}

#[tokio::test]
async fn delete_folder_removes_feeds_and_articles_in_folder() {
    let pool = setup_pool().await;
    sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (30, 'https://example.com/folder-feed', 'Folder Feed', NULL, 123, NULL, 2, 0, 'https://example.com/folder-feed', 0, 0, NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (300, 'Folder Delete Article', 'content', 'Author', NULL, NULL, NULL, 30, NULL, 'guid-300', 'guid-hash-300', 200, NULL, NULL, 100, 0, 0, 1, 100, 'https://example.com/folder-feed/1', 'summary')")
        .execute(&pool)
        .await
        .unwrap();

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/index.php/apps/news/api/v1-3/folders/2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let remaining_folder: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM folder WHERE id = 2")
        .fetch_one(&pool)
        .await
        .unwrap();
    let remaining_feed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feed WHERE id = 30")
        .fetch_one(&pool)
        .await
        .unwrap();
    let remaining_articles: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = 30")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(remaining_folder, 0);
    assert_eq!(remaining_feed, 0);
    assert_eq!(remaining_articles, 0);
}
