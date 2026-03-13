use axum::body::Body;
use axum::http::Request;
use serde_json::Value;
use tower::ServiceExt;

use crate::api::app;

use super::support::{setup_pool, start_fixture_feed_server, state, state_with_testing_mode};

#[tokio::test]
async fn feeds_endpoint_maps_root_folder_to_null() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .uri("/index.php/apps/news/api/v1-3/feeds")
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
    assert_eq!(parsed["feeds"][0]["folderId"], Value::Null);
}

#[tokio::test]
async fn add_feed_blocks_localhost_when_not_testing_mode() {
    let response = app(state_with_testing_mode(setup_pool().await, false))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"url":"http://127.0.0.1:9999/feed.xml","folderId":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Access to localhost is not allowed.");
}

#[tokio::test]
async fn add_feed_duplicate_returns_409() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"url":"https://example.com/rss","folderId":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 409);
}

#[tokio::test]
async fn add_feed_with_missing_folder_returns_422() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"url":"https://example.com/new.xml","folderId":9999}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 422);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Folder with ID 9999 does not exist");
}

#[tokio::test]
async fn delete_nonexistent_feed_returns_404() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/index.php/apps/news/api/v1-3/feeds/9999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn v1_3_move_feed_with_missing_folder_returns_422() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds/10/move")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"folderId":9999}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 422);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Folder with ID 9999 does not exist");
}

#[tokio::test]
async fn v1_2_move_feed_updates_folder() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/feeds/10/move")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"folderId":2}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn v1_2_rename_feed_updates_title() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/feeds/10/rename")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"feedTitle":"Renamed v1-2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM feed WHERE id = 10")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title.as_deref(), Some("Renamed v1-2"));
}

#[tokio::test]
async fn v1_3_rename_feed_updates_title() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds/10/rename")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"feedTitle":"Renamed v1-3"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let title: Option<String> = sqlx::query_scalar("SELECT title FROM feed WHERE id = 10")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title.as_deref(), Some("Renamed v1-3"));
}

#[tokio::test]
async fn v1_2_rename_feed_post_method_returns_405() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-2/feeds/10/rename")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"feedTitle":"Wrong Method"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 405);
}

#[tokio::test]
async fn v1_3_rename_feed_put_method_returns_405() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-3/feeds/10/rename")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"feedTitle":"Wrong Method"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 405);
}

#[tokio::test]
async fn v1_2_mark_feed_items_read_updates_article() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/feeds/10/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":100}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let unread: i64 = sqlx::query_scalar("SELECT unread FROM article WHERE id = 100")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unread, 0);
}

#[tokio::test]
async fn v1_3_mark_feed_items_read_put_method_returns_405() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-3/feeds/10/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":100}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 405);
}

#[tokio::test]
async fn v1_3_mark_feed_items_read_updates_article() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds/10/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":100}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn v1_2_rename_missing_feed_returns_404_with_detail() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/index.php/apps/news/api/v1-2/feeds/9999/rename")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"feedTitle":"Missing"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Feed 9999 not found");
}

#[tokio::test]
async fn v1_3_read_missing_feed_returns_404_with_detail() {
    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds/9999/read")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"newestItemId":100}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["detail"], "Feed 9999 not found");
}

#[tokio::test]
async fn add_feed_success_returns_expected_payload_fields() {
    let feed_url = start_fixture_feed_server().await;

    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","folderId":0}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();

    let created_id = parsed["newestItemId"].as_i64().unwrap();
    let feeds = parsed["feeds"].as_array().unwrap();
    let created_feed = feeds
        .iter()
        .find(|feed| feed["id"].as_i64() == Some(created_id))
        .unwrap();

    assert_eq!(created_feed["url"], feed_url);
    assert_eq!(created_feed["title"], "Fixture Feed");
    assert_eq!(created_feed["link"], "http://example.org/");
    assert_eq!(created_feed["updateErrorCount"], 0);
    assert!(created_feed["nextUpdateTime"].as_i64().is_some());
    assert_eq!(created_feed["folderId"], Value::Null);
}

#[tokio::test]
async fn add_feed_extracts_media_thumbnail_from_body_content() {
    let feed_url = start_fixture_feed_server().await;
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","folderId":0}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let created_feed_id: i64 = sqlx::query_scalar("SELECT id FROM feed WHERE url = ?")
        .bind(&feed_url)
        .fetch_one(&pool)
        .await
        .unwrap();

    let thumbnail: Option<String> =
        sqlx::query_scalar("SELECT media_thumbnail FROM article WHERE feed_id = ? LIMIT 1")
            .bind(created_feed_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        thumbnail.as_deref(),
        Some("https://example.org/entry-thumb.jpg")
    );
}

#[tokio::test]
async fn add_feed_unreadable_source_returns_422() {
    let feed_url = start_fixture_feed_server()
        .await
        .replace("/atom.xml", "/missing.xml");

    let response = app(state(setup_pool().await))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/index.php/apps/news/api/v1-3/feeds")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"url":"{feed_url}","folderId":0}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 422);
}

#[tokio::test]
async fn delete_feed_removes_associated_articles() {
    let pool = setup_pool().await;

    let response = app(state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/index.php/apps/news/api/v1-3/feeds/10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let remaining_feed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM feed WHERE id = 10")
        .fetch_one(&pool)
        .await
        .unwrap();
    let remaining_articles: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(remaining_feed, 0);
    assert_eq!(remaining_articles, 0);
}
