use std::sync::Arc;

use axum::Router as AxumRouter;
use axum::body::Body;
use axum::http::Request;
use axum::http::header as http_header;
use axum::routing::get as axum_get;
use base64::Engine;
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::net::TcpListener;
use tower::ServiceExt;

use crate::config::Config;

use super::{AppState, app};

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query(
            "CREATE TABLE folder (id INTEGER PRIMARY KEY NOT NULL, name VARCHAR NOT NULL UNIQUE, is_root BOOLEAN DEFAULT 0 NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE feed (id INTEGER PRIMARY KEY NOT NULL, url VARCHAR NOT NULL UNIQUE, title VARCHAR, favicon_link VARCHAR, added INTEGER NOT NULL, next_update_time INTEGER, folder_id INTEGER NOT NULL, ordering INTEGER NOT NULL, link VARCHAR, pinned BOOLEAN NOT NULL, update_error_count INTEGER NOT NULL, last_update_error VARCHAR, is_mailing_list BOOLEAN NOT NULL DEFAULT 0, last_quality_check INTEGER, use_extracted_fulltext BOOLEAN NOT NULL DEFAULT 0, use_llm_summary BOOLEAN NOT NULL DEFAULT 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE article (id INTEGER PRIMARY KEY NOT NULL, title VARCHAR, content VARCHAR, author VARCHAR, content_hash VARCHAR, enclosure_link VARCHAR, enclosure_mime VARCHAR, feed_id INTEGER NOT NULL, fingerprint VARCHAR, guid VARCHAR NOT NULL, guid_hash VARCHAR NOT NULL, last_modified INTEGER NOT NULL, media_description VARCHAR, media_thumbnail VARCHAR, pub_date INTEGER, rtl BOOLEAN NOT NULL, starred BOOLEAN NOT NULL, unread BOOLEAN NOT NULL, updated_date INTEGER, url VARCHAR, summary VARCHAR)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO folder (id, name, is_root) VALUES (1, '', 1), (2, 'Tech', 0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (10, 'https://example.com/rss', 'Example Feed', NULL, 123, NULL, 1, 0, 'https://example.com', 0, 0, NULL, 0, NULL, 0, 0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO article (id, title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (100, 'Article 1', 'full content', 'Author', NULL, NULL, NULL, 10, NULL, 'guid-1', 'guid-hash-1', 200, NULL, NULL, 100, 0, 0, 1, 100, 'https://example.com/article', 'summary content')")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    async fn start_fixture_feed_server() -> String {
        let app = AxumRouter::new().route(
            "/atom.xml",
            axum_get(|| async {
                (
                    [(http_header::CONTENT_TYPE, "application/atom+xml")],
                    r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Fixture Feed</title>
  <link href="http://example.org/" />
  <updated>2026-03-06T00:00:00Z</updated>
  <id>tag:example.org,2026:feed</id>
  <entry>
    <title>Entry One</title>
    <link href="http://example.org/entry-one" />
    <id>tag:example.org,2026:entry1</id>
    <updated>2026-03-06T00:00:00Z</updated>
    <summary>Entry summary</summary>
        <content type="html"><![CDATA[<p>Entry content</p><img src="https://example.org/entry-thumb.jpg" alt="thumb" />]]></content>
  </entry>
</feed>"#,
                )
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        format!("http://{addr}/atom.xml")
    }

    fn state(pool: SqlitePool) -> AppState {
        AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
                feed_update_frequency_min: 15,
                openai_api_key: None,
                openai_base_url: "https://api.openai.com/v1".to_string(),
                openai_model: "gpt-5-nano".to_string(),
                testing_mode: true,
            }),
            feed_http_client: crate::http_client::build_feed_http_client().unwrap(),
            article_http_client: crate::http_client::build_article_http_client().unwrap(),
        }
    }

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
                    .uri(
                        "/index.php/apps/news/api/v1-3/items/updated?lastModified=180&type=0&id=10",
                    )
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
        let app = app(state(setup_pool().await));

        let response = app
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
        let app = app(state(setup_pool().await));

        let response = app
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
    async fn protected_endpoints_require_auth_when_configured() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
                feed_update_frequency_min: 15,
                openai_api_key: None,
                openai_base_url: "https://api.openai.com/v1".to_string(),
                openai_model: "gpt-5-nano".to_string(),
                testing_mode: true,
            }),
            feed_http_client: crate::http_client::build_feed_http_client().unwrap(),
            article_http_client: crate::http_client::build_article_http_client().unwrap(),
        };

        let response = app(state)
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
    async fn add_feed_blocks_localhost_when_not_testing_mode() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
                feed_update_frequency_min: 15,
                openai_api_key: None,
                openai_base_url: "https://api.openai.com/v1".to_string(),
                openai_model: "gpt-5-nano".to_string(),
                testing_mode: false,
            }),
            feed_http_client: crate::http_client::build_feed_http_client().unwrap(),
            article_http_client: crate::http_client::build_article_http_client().unwrap(),
        };

        let response = app(state)
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
        let app = app(state(setup_pool().await));

        let response = app
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
        let app = app(state(setup_pool().await));

        let response = app
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
        let app = app(state(setup_pool().await));

        let response = app
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
    async fn protected_endpoints_reject_invalid_credentials() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: Some("testuser".to_string()),
                password: Some("testpass".to_string()),
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
                feed_update_frequency_min: 15,
                openai_api_key: None,
                openai_base_url: "https://api.openai.com/v1".to_string(),
                openai_model: "gpt-5-nano".to_string(),
                testing_mode: true,
            }),
            feed_http_client: crate::http_client::build_feed_http_client().unwrap(),
            article_http_client: crate::http_client::build_article_http_client().unwrap(),
        };

        let wrong = base64::engine::general_purpose::STANDARD.encode("wronguser:wrongpass");
        let response = app(state)
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
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: Some("testuser".to_string()),
                password: Some("testpass".to_string()),
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
                feed_update_frequency_min: 15,
                openai_api_key: None,
                openai_base_url: "https://api.openai.com/v1".to_string(),
                openai_model: "gpt-5-nano".to_string(),
                testing_mode: true,
            }),
            feed_http_client: crate::http_client::build_feed_http_client().unwrap(),
            article_http_client: crate::http_client::build_article_http_client().unwrap(),
        };

        let ok = base64::engine::general_purpose::STANDARD.encode("testuser:testpass");
        let response = app(state)
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
                        r#"{{"url":"{feed_url}","folderId":0}}"#,
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
            .find(|f| f["id"].as_i64() == Some(created_id))
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
                        r#"{{"url":"{feed_url}","folderId":0}}"#,
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
                        r#"{{"url":"{feed_url}","folderId":0}}"#,
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 422);
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
    async fn delete_feed_removes_associated_articles() {
        let pool = setup_pool().await;
        let app = app(state(pool.clone()));

        let response = app
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

        let app = app(state(pool.clone()));
        let response = app
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
