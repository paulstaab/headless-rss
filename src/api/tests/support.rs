use std::sync::Arc;

use axum::Router as AxumRouter;
use axum::http::header as http_header;
use axum::routing::get as axum_get;
use sqlx::SqlitePool;
use tokio::net::TcpListener;

use crate::config::Config;

use super::super::AppState;

/// Creates an in-memory database populated with the minimum API fixture data.
pub(super) async fn setup_pool() -> SqlitePool {
    let pool = crate::db::create_memory_pool().await.unwrap();

    sqlx::query("UPDATE folder SET id = 1 WHERE id = 0")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO folder (id, name, is_root) VALUES (2, 'Tech', 0)")
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

/// Starts a disposable HTTP fixture server that serves a small Atom feed.
pub(super) async fn start_fixture_feed_server() -> String {
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

/// Builds application state for API tests with default auth and SSRF test settings.
pub(super) fn state(pool: SqlitePool) -> AppState {
    build_state(pool, None, None, true)
}

/// Builds application state for API tests with basic auth enabled.
pub(super) fn state_with_auth(pool: SqlitePool, username: &str, password: &str) -> AppState {
    build_state(pool, Some(username), Some(password), true)
}

/// Builds application state for API tests with a custom SSRF testing-mode flag.
pub(super) fn state_with_testing_mode(pool: SqlitePool, testing_mode: bool) -> AppState {
    build_state(pool, None, None, testing_mode)
}

fn build_state(
    pool: SqlitePool,
    username: Option<&str>,
    password: Option<&str>,
    testing_mode: bool,
) -> AppState {
    AppState {
        pool,
        config: Arc::new(Config {
            username: username.map(str::to_string),
            password: password.map(str::to_string),
            version: "dev".to_string(),
            db_path: "data/headless-rss.sqlite3".to_string(),
            feed_update_frequency_min: 15,
            openai_api_key: None,
            openai_base_url: "https://api.openai.com/v1".to_string(),
            openai_model: "gpt-5-nano".to_string(),
            openai_timeout_seconds: 30,
            testing_mode,
        }),
        feed_http_client: crate::http_client::build_feed_http_client().unwrap(),
        article_http_client: crate::http_client::build_article_http_client().unwrap(),
    }
}
