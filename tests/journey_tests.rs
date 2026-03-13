//! Black-box journey tests for the Rust service.
//!
//! These scenarios start the compiled binary as an external process, use an
//! empty temporary SQLite database, mock remote feed sources, and verify the
//! end-to-end behavior through HTTP APIs and CLI update commands.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{fs, io};

use axum::Router;
use axum::http::header;
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use reqwest::{Client, StatusCode};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use tokio::net::TcpListener as TokioTcpListener;

const API_V13: &str = "/index.php/apps/news/api/v1-3";
const API_V12: &str = "/index.php/apps/news/api/v1-2";

struct RunningServer {
    child: Child,
}

struct ScenarioContext {
    _temp_dir: TempDir,
    db_path: PathBuf,
    base_url: String,
    client: Client,
    _server: RunningServer,
    _mock_feed_base_url: String,
    feed_urls: Vec<String>,
    auth: Option<(String, String)>,
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn ts_e2e_001_start_service_and_check_health() {
    let context = setup_context(None).await;

    let status_before = context
        .client
        .get(format!("{}/status", context.base_url))
        .send()
        .await
        .expect("status request failed");
    assert_eq!(status_before.status(), StatusCode::OK);

    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let status_after = context
        .client
        .get(format!("{}/status", context.base_url))
        .send()
        .await
        .expect("status request failed");
    assert_eq!(status_after.status(), StatusCode::OK);
}

#[tokio::test]
async fn ts_e2e_002_verify_nextcloud_compatibility_baseline() {
    let context = setup_context(None).await;

    let version = get_json(&context, &format!("{API_V13}/version"), None).await;
    assert!(version["version"].is_string());

    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    assert_eq!(feeds["feeds"].as_array().map_or(0, Vec::len), 4);

    let folders = get_json(&context, &format!("{API_V13}/folders"), None).await;
    assert_eq!(folders["folders"].as_array().map_or(0, Vec::len), 0);
}

#[tokio::test]
async fn ts_e2e_003_add_feeds_then_run_update_cycle() {
    let context = setup_context(None).await;

    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let feeds: serde_json::Value = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let feed_count = feeds["feeds"].as_array().map_or(0, Vec::len);
    assert_eq!(feed_count, 4, "expected all shared feeds to exist");

    let items: serde_json::Value =
        get_json(&context, &format!("{API_V13}/items?type=3&id=0"), None).await;
    let item_count = items["items"].as_array().map_or(0, Vec::len);
    assert!(item_count >= 4, "expected at least one item per feed");
}

#[tokio::test]
async fn ts_e2e_004_add_feeds_into_non_root_folder() {
    let context = setup_context(None).await;

    let folder = post_json(
        &context,
        &format!("{API_V13}/folders"),
        json!({ "name": "tech" }),
        None,
    )
    .await;
    let folder_id = folder["folders"][0]["id"]
        .as_i64()
        .expect("missing folder id");

    for url in context.feed_urls.iter().take(3) {
        let response = post_json(
            &context,
            &format!("{API_V13}/feeds"),
            json!({ "url": url, "folderId": folder_id }),
            None,
        )
        .await;
        assert_eq!(response["feeds"][0]["folderId"].as_i64(), Some(folder_id));
    }

    let root_feed = context.feed_urls[3].clone();
    post_json(
        &context,
        &format!("{API_V13}/feeds"),
        json!({ "url": root_feed }),
        None,
    )
    .await;

    run_update_cycle(&context).await;

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let root_count = feeds["feeds"]
        .as_array()
        .expect("missing feeds array")
        .iter()
        .filter(|f| f["folderId"].is_null())
        .count();
    assert!(root_count >= 1);
}

#[tokio::test]
async fn ts_e2e_005_prevent_duplicate_feed_add() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let duplicate = post_json_status(
        &context,
        &format!("{API_V13}/feeds"),
        json!({ "url": context.feed_urls[0] }),
        None,
    )
    .await;
    assert_eq!(duplicate, StatusCode::CONFLICT);
}

#[tokio::test]
async fn ts_e2e_006_fetch_unread_and_mark_one_read() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let unread_before = get_json(
        &context,
        &format!("{API_V13}/items?type=3&id=0&getRead=false"),
        None,
    )
    .await;
    let item_id = unread_before["items"][0]["id"]
        .as_i64()
        .expect("missing item id");

    post_no_body_status(&context, &format!("{API_V13}/items/{item_id}/read"), None).await;

    let unread_after = get_json(
        &context,
        &format!("{API_V13}/items?type=3&id=0&getRead=false"),
        None,
    )
    .await;
    let unread_ids: Vec<i64> = unread_after["items"]
        .as_array()
        .expect("missing items array")
        .iter()
        .filter_map(|item| item["id"].as_i64())
        .collect();
    assert!(!unread_ids.contains(&item_id));
}

#[tokio::test]
async fn ts_e2e_007_mark_feed_items_read_to_boundary() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let feed_id = feeds["feeds"][0]["id"].as_i64().expect("missing feed id");
    let feed_items = get_json(
        &context,
        &format!("{API_V13}/items?type=0&id={feed_id}&getRead=true"),
        None,
    )
    .await;
    let newest_item_id = feed_items["items"][0]["id"]
        .as_i64()
        .expect("missing item id");

    post_json(
        &context,
        &format!("{API_V13}/feeds/{feed_id}/read"),
        json!({ "newestItemId": newest_item_id }),
        None,
    )
    .await;

    let unread = get_json(
        &context,
        &format!("{API_V13}/items?type=0&id={feed_id}&getRead=false"),
        None,
    )
    .await;
    assert_eq!(unread["items"].as_array().map_or(0, Vec::len), 0);
}

#[tokio::test]
async fn ts_e2e_008_incremental_sync_with_updated_items() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let all_items = get_json(&context, &format!("{API_V13}/items?type=3&id=0"), None).await;
    let item_id = all_items["items"][0]["id"]
        .as_i64()
        .expect("missing item id");
    let baseline = all_items["items"][0]["lastModified"]
        .as_i64()
        .expect("missing lastModified");

    post_no_body_status(&context, &format!("{API_V13}/items/{item_id}/star"), None).await;
    let updated = get_json(
        &context,
        &format!("{API_V13}/items/updated?lastModified={baseline}&type=3&id=0"),
        None,
    )
    .await;
    assert!(updated["items"].as_array().map_or(0, Vec::len) >= 1);
}

#[tokio::test]
async fn ts_e2e_009_star_and_unstar_item_v1_3() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let all_items = get_json(&context, &format!("{API_V13}/items?type=3&id=0"), None).await;
    let item_id = all_items["items"][0]["id"]
        .as_i64()
        .expect("missing item id");

    post_no_body_status(&context, &format!("{API_V13}/items/{item_id}/star"), None).await;
    let starred = get_json(&context, &format!("{API_V13}/items?type=2&id=0"), None).await;
    assert!(starred["items"].as_array().map_or(0, Vec::len) >= 1);

    post_no_body_status(&context, &format!("{API_V13}/items/{item_id}/unstar"), None).await;
    let unstarred = get_json(&context, &format!("{API_V13}/items?type=2&id=0"), None).await;
    let starred_ids: Vec<i64> = unstarred["items"]
        .as_array()
        .expect("missing items array")
        .iter()
        .filter_map(|item| item["id"].as_i64())
        .collect();
    assert!(!starred_ids.contains(&item_id));
}

#[tokio::test]
async fn ts_e2e_010_star_by_guid_hash_v1_2() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let all_items = get_json(&context, &format!("{API_V13}/items?type=3&id=0"), None).await;
    let item = &all_items["items"][0];
    let feed_id = item["feedId"].as_i64().expect("missing feedId");
    let guid_hash = item["guidHash"]
        .as_str()
        .expect("missing guidHash")
        .to_string();

    let status = put_json_status(
        &context,
        &format!("{API_V12}/items/{feed_id}/{guid_hash}/star"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let starred = get_json(&context, &format!("{API_V13}/items?type=2&id=0"), None).await;
    assert!(starred["items"].as_array().map_or(0, Vec::len) >= 1);
}

#[tokio::test]
async fn ts_e2e_011_rename_folder_keeps_feed_associations() {
    let context = setup_context(None).await;
    let folder = post_json(
        &context,
        &format!("{API_V13}/folders"),
        json!({ "name": "alpha" }),
        None,
    )
    .await;
    let folder_id = folder["folders"][0]["id"]
        .as_i64()
        .expect("missing folder id");

    post_json(
        &context,
        &format!("{API_V13}/feeds"),
        json!({ "url": context.feed_urls[0], "folderId": folder_id }),
        None,
    )
    .await;
    run_update_cycle(&context).await;

    let status = put_json_status(
        &context,
        &format!("{API_V13}/folders/{folder_id}"),
        json!({ "name": "beta" }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    assert_eq!(feeds["feeds"][0]["folderId"].as_i64(), Some(folder_id));
}

#[tokio::test]
async fn ts_e2e_012_move_feed_between_folders() {
    let context = setup_context(None).await;
    let folder_a = post_json(
        &context,
        &format!("{API_V13}/folders"),
        json!({ "name": "A" }),
        None,
    )
    .await;
    let folder_b = post_json(
        &context,
        &format!("{API_V13}/folders"),
        json!({ "name": "B" }),
        None,
    )
    .await;
    let folder_a_id = folder_a["folders"][0]["id"]
        .as_i64()
        .expect("missing folder id");
    let folder_b_id = folder_b["folders"][0]["id"]
        .as_i64()
        .expect("missing folder id");

    let add = post_json(
        &context,
        &format!("{API_V13}/feeds"),
        json!({ "url": context.feed_urls[0], "folderId": folder_a_id }),
        None,
    )
    .await;
    let feed_id = add["feeds"][0]["id"].as_i64().expect("missing feed id");

    let status = post_json_status(
        &context,
        &format!("{API_V13}/feeds/{feed_id}/move"),
        json!({ "folderId": folder_b_id }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let moved = feeds["feeds"]
        .as_array()
        .expect("missing feeds")
        .iter()
        .find(|f| f["id"].as_i64() == Some(feed_id))
        .expect("feed not found after move");
    assert_eq!(moved["folderId"].as_i64(), Some(folder_b_id));
}

#[tokio::test]
async fn ts_e2e_013_delete_feed_with_cascade() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let feed_id = feeds["feeds"][0]["id"].as_i64().expect("missing feed id");

    let status = delete_status(&context, &format!("{API_V13}/feeds/{feed_id}"), None).await;
    assert_eq!(status, StatusCode::OK);

    let remaining = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let remaining_ids: Vec<i64> = remaining["feeds"]
        .as_array()
        .expect("missing feeds")
        .iter()
        .filter_map(|f| f["id"].as_i64())
        .collect();
    assert!(!remaining_ids.contains(&feed_id));

    let items_for_feed = get_json(
        &context,
        &format!("{API_V13}/items?type=0&id={feed_id}"),
        None,
    )
    .await;
    assert_eq!(items_for_feed["items"].as_array().map_or(0, Vec::len), 0);
}

#[tokio::test]
async fn ts_e2e_014_delete_folder_with_cascade() {
    let context = setup_context(None).await;
    let folder = post_json(
        &context,
        &format!("{API_V13}/folders"),
        json!({ "name": "drop-me" }),
        None,
    )
    .await;
    let folder_id = folder["folders"][0]["id"]
        .as_i64()
        .expect("missing folder id");

    for url in context.feed_urls.iter().take(2) {
        post_json(
            &context,
            &format!("{API_V13}/feeds"),
            json!({ "url": url, "folderId": folder_id }),
            None,
        )
        .await;
    }
    post_json(
        &context,
        &format!("{API_V13}/feeds"),
        json!({ "url": context.feed_urls[2] }),
        None,
    )
    .await;

    run_update_cycle(&context).await;

    let status = delete_status(&context, &format!("{API_V13}/folders/{folder_id}"), None).await;
    assert_eq!(status, StatusCode::OK);

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let feeds_in_folder = feeds["feeds"]
        .as_array()
        .expect("missing feeds")
        .iter()
        .filter(|feed| feed["folderId"].as_i64() == Some(folder_id))
        .count();
    assert_eq!(feeds_in_folder, 0);
}

#[tokio::test]
async fn ts_e2e_015_auth_disabled_allows_anonymous_access() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let status = context
        .client
        .get(format!("{}{API_V13}/feeds", context.base_url))
        .send()
        .await
        .expect("feeds request failed")
        .status();
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn ts_e2e_016_auth_enforces_credentials_and_allows_update() {
    let username = "journey-user".to_string();
    let password = "journey-pass".to_string();
    let context = setup_context(Some((&username, &password))).await;

    let unauthorized = add_feed(
        &context.client,
        &context.base_url,
        &context.feed_urls[0],
        None,
    )
    .await;
    assert_eq!(unauthorized, StatusCode::UNAUTHORIZED);

    let authorized = add_feed(
        &context.client,
        &context.base_url,
        &context.feed_urls[0],
        auth_for(&context),
    )
    .await;
    assert_eq!(authorized, StatusCode::OK);

    run_update_cycle(&context).await;

    let (user, pass) = auth_for(&context).expect("missing auth tuple");
    let response = context
        .client
        .get(format!("{}{API_V13}/feeds", context.base_url))
        .header("Authorization", basic_auth_value(user, pass))
        .send()
        .await
        .expect("authorized feeds request failed");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
/// Verifies the full subprocess flow for mocked mailbox credential storage and newsletter ingestion.
async fn ts_e2e_017_store_email_credentials_and_ingest_newsletters() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;
    run_update_cycle(&context).await;

    let mock_messages_path = write_mock_imap_messages(context._temp_dir.path())
        .expect("failed to write mock IMAP messages");

    let status = Command::new(binary_path())
        .arg("add-email-credentials")
        .arg("--server")
        .arg("mock-imap.local")
        .arg("--port")
        .arg("993")
        .arg("--username")
        .arg("journey@example.com")
        .arg("--password")
        .arg("journey-secret")
        .env("DATABASE_PATH", &context.db_path)
        .env("TESTING_MODE", "true")
        .env("HEADLESS_RSS_TEST_IMAP_ALLOW", "true")
        .env("HEADLESS_RSS_TEST_IMAP_MESSAGES_FILE", &mock_messages_path)
        .status()
        .expect("failed to run add-email-credentials command");
    assert!(status.success());

    mark_all_feeds_due(&context.db_path).await;
    run_update_command_with_env(
        &context.db_path,
        auth_for(&context).map(|(u, _)| u),
        auth_for(&context).map(|(_, p)| p),
        &[
            ("HEADLESS_RSS_TEST_IMAP_ALLOW", "true"),
            (
                "HEADLESS_RSS_TEST_IMAP_MESSAGES_FILE",
                mock_messages_path.to_string_lossy().as_ref(),
            ),
        ],
    )
    .await;

    let feeds = get_json(&context, &format!("{API_V13}/feeds"), None).await;
    let newsletter_feed = feeds["feeds"]
        .as_array()
        .and_then(|items| {
            items.iter().find(|feed| {
                feed["title"].as_str() == Some("Example List")
                    && feed["url"].as_str() == Some("list@example.com")
            })
        })
        .expect("missing newsletter feed");
    let newsletter_feed_id = newsletter_feed["id"]
        .as_i64()
        .expect("missing newsletter feed id");

    let newsletter_items = get_json(
        &context,
        &format!("{API_V13}/items?type=0&id={newsletter_feed_id}"),
        None,
    )
    .await;
    let items = newsletter_items["items"]
        .as_array()
        .expect("missing newsletter items array");
    assert_eq!(items.len(), 2);
    let titles: Vec<&str> = items
        .iter()
        .filter_map(|item| item["title"].as_str())
        .collect();
    assert!(titles.contains(&"Newsletter Digest"));
    assert!(titles.contains(&"Second Newsletter"));
}

#[tokio::test]
async fn ts_e2e_018_updater_handles_source_failures_gracefully() {
    let context = setup_context(None).await;
    add_shared_feeds(&context).await;

    insert_feed_row(&context.db_path, "http://127.0.0.1:9/does-not-exist", 0).await;
    run_update_cycle(&context).await;

    let options = SqliteConnectOptions::new()
        .filename(&context.db_path)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to connect sqlite");

    let err_count: i64 = sqlx::query_scalar(
        "SELECT update_error_count FROM feed WHERE url = 'http://127.0.0.1:9/does-not-exist'",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to query error count");
    assert!(err_count > 0);

    pool.close().await;
}

#[tokio::test]
async fn ts_e2e_019_restart_with_persistent_database() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let db_path = temp_dir.path().join("journey-restart.sqlite3");

    let mock_feed_base_url = start_mock_feed_server().await;
    let feed_urls = shared_feed_urls(&mock_feed_base_url);

    let port = find_free_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let server = spawn_server(&db_path, port, None, None, &[]);
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to create http client");
    wait_for_ready(&client, &base_url).await;

    for url in &feed_urls {
        let status = add_feed(&client, &base_url, url, None).await;
        assert_eq!(status, StatusCode::OK, "feed add failed for {url}");
    }

    mark_all_feeds_due(&db_path).await;
    run_update_command(&db_path, None, None).await;

    let feeds_before =
        get_json_from_client(&client, &base_url, &format!("{API_V13}/feeds"), None).await;
    let count_before = feeds_before["feeds"].as_array().map_or(0, Vec::len);

    drop(server);

    let port_after = find_free_port();
    let base_url_after = format!("http://127.0.0.1:{port_after}");
    let _server_after = spawn_server(&db_path, port_after, None, None, &[]);
    wait_for_ready(&client, &base_url_after).await;

    let feeds_after =
        get_json_from_client(&client, &base_url_after, &format!("{API_V13}/feeds"), None).await;
    let count_after = feeds_after["feeds"].as_array().map_or(0, Vec::len);
    assert_eq!(count_before, count_after);

    drop(temp_dir);
}

#[tokio::test]
async fn ts_e2e_020_generates_llm_summary_from_extracted_article_content() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let db_path = temp_dir.path().join("journey-llm.sqlite3");

    let fixture_base_url = start_mock_llm_fixture_server().await;
    let feed_url = format!("{fixture_base_url}/llm.xml");

    let port = find_free_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let server = spawn_server(
        &db_path,
        port,
        None,
        None,
        &[
            ("OPENAI_API_KEY", "journey-test-key"),
            ("OPENAI_BASE_URL", &format!("{fixture_base_url}/v1")),
            ("OPENAI_MODEL", "journey-summary-model"),
        ],
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to create http client");
    wait_for_ready(&client, &base_url).await;

    let status = add_feed(&client, &base_url, &feed_url, None).await;
    assert_eq!(status, StatusCode::OK, "feed add failed for {feed_url}");

    let items = get_json_from_client(
        &client,
        &base_url,
        &format!("{API_V13}/items?type=3&id=0"),
        None,
    )
    .await;
    let item = items["items"]
        .as_array()
        .and_then(|entries| entries.first())
        .expect("missing inserted item");
    let item_id = item["id"].as_i64().expect("missing item id");

    assert_eq!(
        item["body"].as_str(),
        Some("Fixture summary from mock LLM. (AI generated)")
    );

    let content = get_json_from_client(
        &client,
        &base_url,
        &format!("{API_V13}/items/{item_id}/content"),
        None,
    )
    .await;
    let content_body = content["content"]
        .as_str()
        .expect("missing item content body");
    assert!(
        content_body.contains("important technical details"),
        "expected extracted article content, got: {content_body}"
    );
    assert!(
        !content_body.contains("Cookie banner"),
        "expected boilerplate removal, got: {content_body}"
    );

    drop(server);
    drop(temp_dir);
}

async fn setup_context(auth: Option<(&str, &str)>) -> ScenarioContext {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let db_path = temp_dir.path().join("journey.sqlite3");
    let auth_in = auth;

    let mock_feed_base_url = start_mock_feed_server().await;
    let feed_urls = shared_feed_urls(&mock_feed_base_url);

    let port = find_free_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let server = spawn_server(
        &db_path,
        port,
        auth_in.map(|a| a.0),
        auth_in.map(|a| a.1),
        &[],
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to create http client");
    wait_for_ready(&client, &base_url).await;

    ScenarioContext {
        _temp_dir: temp_dir,
        db_path,
        base_url,
        client,
        _server: server,
        _mock_feed_base_url: mock_feed_base_url,
        feed_urls,
        auth: auth_in.map(|(u, p)| (u.to_string(), p.to_string())),
    }
}

fn shared_feed_urls(base_url: &str) -> Vec<String> {
    vec![
        format!("{base_url}/tagesschau.xml"),
        format!("{base_url}/heise.xml"),
        format!("{base_url}/heise-top.xml"),
        format!("{base_url}/simon.xml"),
    ]
}

fn auth_for(context: &ScenarioContext) -> Option<(&str, &str)> {
    context.auth.as_ref().map(|(u, p)| (u.as_str(), p.as_str()))
}

async fn add_shared_feeds(context: &ScenarioContext) {
    for url in &context.feed_urls {
        let status = add_feed(&context.client, &context.base_url, url, auth_for(context)).await;
        assert_eq!(status, StatusCode::OK, "feed add failed for {url}");
    }
}

async fn run_update_cycle(context: &ScenarioContext) {
    mark_all_feeds_due(&context.db_path).await;
    run_update_command(
        &context.db_path,
        auth_for(context).map(|(u, _)| u),
        auth_for(context).map(|(_, p)| p),
    )
    .await;
}

async fn insert_feed_row(db_path: &Path, url: &str, folder_id: i64) {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to connect sqlite");

    sqlx::query(
        "INSERT INTO feed (url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (?, ?, NULL, CAST(strftime('%s','now') AS INTEGER), 0, ?, 0, NULL, 0, 0, NULL)",
    )
    .bind(url)
    .bind("Broken Feed")
    .bind(folder_id)
    .execute(&pool)
    .await
    .expect("failed to insert feed row");

    pool.close().await;
}

async fn get_json(
    context: &ScenarioContext,
    path: &str,
    credentials: Option<(&str, &str)>,
) -> serde_json::Value {
    get_json_from_client(&context.client, &context.base_url, path, credentials).await
}

async fn get_json_from_client(
    client: &Client,
    base_url: &str,
    path: &str,
    credentials: Option<(&str, &str)>,
) -> serde_json::Value {
    let url = format!("{base_url}{path}");
    let mut request = client.get(url);
    if let Some((u, p)) = credentials {
        request = request.header("Authorization", basic_auth_value(u, p));
    }

    let response = request
        .send()
        .await
        .expect("request failed")
        .error_for_status()
        .expect("endpoint returned non-success");
    let body = response.text().await.expect("failed to read body");
    serde_json::from_str(&body).expect("failed to parse json body")
}

async fn post_json(
    context: &ScenarioContext,
    path: &str,
    body: serde_json::Value,
    credentials: Option<(&str, &str)>,
) -> serde_json::Value {
    let url = format!("{}{}", context.base_url, path);
    let mut request = context
        .client
        .post(url)
        .header("content-type", "application/json")
        .body(body.to_string());
    if let Some((u, p)) = credentials {
        request = request.header("Authorization", basic_auth_value(u, p));
    }

    let response = request
        .send()
        .await
        .expect("request failed")
        .error_for_status()
        .expect("endpoint returned non-success");
    let body = response.text().await.expect("failed to read body");
    if body.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&body).expect("failed to parse json body")
    }
}

async fn post_json_status(
    context: &ScenarioContext,
    path: &str,
    body: serde_json::Value,
    credentials: Option<(&str, &str)>,
) -> StatusCode {
    let url = format!("{}{}", context.base_url, path);
    let mut request = context
        .client
        .post(url)
        .header("content-type", "application/json")
        .body(body.to_string());
    if let Some((u, p)) = credentials {
        request = request.header("Authorization", basic_auth_value(u, p));
    }
    request.send().await.expect("request failed").status()
}

async fn put_json_status(
    context: &ScenarioContext,
    path: &str,
    body: serde_json::Value,
    credentials: Option<(&str, &str)>,
) -> StatusCode {
    let url = format!("{}{}", context.base_url, path);
    let mut request = context
        .client
        .put(url)
        .header("content-type", "application/json")
        .body(body.to_string());
    if let Some((u, p)) = credentials {
        request = request.header("Authorization", basic_auth_value(u, p));
    }
    request.send().await.expect("request failed").status()
}

async fn post_no_body_status(
    context: &ScenarioContext,
    path: &str,
    credentials: Option<(&str, &str)>,
) -> StatusCode {
    post_json_status(context, path, json!({}), credentials).await
}

async fn delete_status(
    context: &ScenarioContext,
    path: &str,
    credentials: Option<(&str, &str)>,
) -> StatusCode {
    let url = format!("{}{}", context.base_url, path);
    let mut request = context.client.delete(url);
    if let Some((u, p)) = credentials {
        request = request.header("Authorization", basic_auth_value(u, p));
    }
    request.send().await.expect("request failed").status()
}

/// Starts the service binary in serve mode using an isolated test database.
fn spawn_server(
    db_path: &Path,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
    extra_envs: &[(&str, &str)],
) -> RunningServer {
    let mut command = Command::new(binary_path());
    command
        .arg("serve")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .env("DATABASE_PATH", db_path)
        .env("TESTING_MODE", "true")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(user) = username {
        command.env("USERNAME", user);
    }
    if let Some(pass) = password {
        command.env("PASSWORD", pass);
    }
    for (key, value) in extra_envs {
        command.env(key, value);
    }

    let child = command.spawn().expect("failed to spawn rust api server");
    RunningServer { child }
}

/// Runs the CLI update command against the same isolated database.
async fn run_update_command(db_path: &Path, username: Option<&str>, password: Option<&str>) {
    run_update_command_with_env(db_path, username, password, &[]).await;
}

/// Runs the CLI update command with extra environment variables for test-only integration hooks.
async fn run_update_command_with_env(
    db_path: &Path,
    username: Option<&str>,
    password: Option<&str>,
    extra_envs: &[(&str, &str)],
) {
    let db_path = db_path.to_path_buf();
    let username = username.map(ToOwned::to_owned);
    let password = password.map(ToOwned::to_owned);
    let extra_envs: Vec<(String, String)> = extra_envs
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();

    let status = tokio::task::spawn_blocking(move || {
        let mut command = Command::new(binary_path());
        command
            .arg("update")
            .env("DATABASE_PATH", &db_path)
            .env("TESTING_MODE", "true")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Some(user) = username.as_deref() {
            command.env("USERNAME", user);
        }
        if let Some(pass) = password.as_deref() {
            command.env("PASSWORD", pass);
        }
        for (key, value) in &extra_envs {
            command.env(key, value);
        }

        command.status()
    })
    .await
    .expect("update command task panicked")
    .expect("failed to run update command");

    assert!(
        status.success(),
        "update command failed with status {status}"
    );
}

/// Writes serialized mock IMAP messages for subprocess-based journey tests.
fn write_mock_imap_messages(base_dir: &Path) -> io::Result<PathBuf> {
    let file_path = base_dir.join("mock-imap-messages.json");
    let messages = vec![
        concat!(
            "Subject: Newsletter Digest\r\n",
            "From: Example List <list@example.com>\r\n",
            "List-Unsubscribe: <mailto:unsubscribe@example.com>\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Body 1"
        ),
        concat!(
            "Subject: Not A Newsletter\r\n",
            "From: Person <person@example.com>\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Ignore me"
        ),
        concat!(
            "Subject: Second Newsletter\r\n",
            "From: Example List <list@example.com>\r\n",
            "List-Unsubscribe: <mailto:unsubscribe@example.com>\r\n",
            "Content-Type: text/plain; charset=utf-8\r\n",
            "\r\n",
            "Body 2"
        ),
    ];
    let encoded = serde_json::to_string(&messages).expect("failed to encode mock messages");
    fs::write(&file_path, encoded)?;
    Ok(file_path)
}

/// Marks feeds as due so the update command processes them in the test run.
async fn mark_all_feeds_due(db_path: &Path) {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to connect to test sqlite db");

    sqlx::query("UPDATE feed SET next_update_time = 0")
        .execute(&pool)
        .await
        .expect("failed to mark feeds as due");

    pool.close().await;
}

/// Waits until the service reports healthy status or panics on timeout.
async fn wait_for_ready(client: &Client, base_url: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match client.get(format!("{base_url}/status")).send().await {
            Ok(response) if response.status() == StatusCode::OK => return,
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("service did not become ready before timeout");
}

/// Adds one feed through the v1-3 API and returns the HTTP status.
async fn add_feed(
    client: &Client,
    base_url: &str,
    feed_url: &str,
    credentials: Option<(&str, &str)>,
) -> StatusCode {
    let mut request = client
        .post(format!("{base_url}{API_V13}/feeds"))
        .header("content-type", "application/json")
        .body(json!({ "url": feed_url }).to_string());

    if let Some((username, password)) = credentials {
        request = request.header("Authorization", basic_auth_value(username, password));
    }

    request
        .send()
        .await
        .expect("add-feed request failed")
        .status()
}

/// Starts a local feed fixture server used by journey tests.
async fn start_mock_feed_server() -> String {
    let tagesschau = atom_feed("tagesschau-feed", "tagesschau-entry", "Tagesschau Entry");
    let heise = atom_feed("heise-feed", "heise-entry", "Heise Entry");
    let heise_top = atom_feed("heise-top-feed", "heise-top-entry", "Heise Top Entry");
    let simon = atom_feed("simon-feed", "simon-entry", "Simon Entry");

    let app = Router::new()
        .route(
            "/tagesschau.xml",
            get(move || {
                let body = tagesschau.clone();
                async move { ([(header::CONTENT_TYPE, "application/atom+xml")], body) }
            }),
        )
        .route(
            "/heise.xml",
            get(move || {
                let body = heise.clone();
                async move { ([(header::CONTENT_TYPE, "application/atom+xml")], body) }
            }),
        )
        .route(
            "/heise-top.xml",
            get(move || {
                let body = heise_top.clone();
                async move { ([(header::CONTENT_TYPE, "application/atom+xml")], body) }
            }),
        )
        .route(
            "/simon.xml",
            get(move || {
                let body = simon.clone();
                async move { ([(header::CONTENT_TYPE, "application/atom+xml")], body) }
            }),
        );

    let listener = TokioTcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind mock feed listener");
    let addr = listener
        .local_addr()
        .expect("failed to get mock feed local address");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock feed server crashed");
    });

    format!("http://{addr}")
}

async fn start_mock_llm_fixture_server() -> String {
    let listener = TokioTcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind mock llm fixture listener");
    let addr = listener
        .local_addr()
        .expect("failed to get mock llm fixture local address");
    let base_url = format!("http://{addr}");

    let app = Router::new()
        .route(
            "/llm.xml",
            get({
                let base_url = base_url.clone();
                move || {
                    let body = llm_atom_feed(&base_url);
                    async move { ([(header::CONTENT_TYPE, "application/atom+xml")], body) }
                }
            }),
        )
        .route(
            "/articles/llm-entry",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    r#"<!doctype html>
<html>
  <body>
    <header>Cookie banner and navigation</header>
    <main>
      <article>
        <h1>Mocked Article</h1>
        <p>This article body contains important technical details about reliable full-text extraction in RSS workflows.</p>
        <p>It is long enough to justify summarization and to verify that the extracted content replaces the short feed teaser.</p>
      </article>
    </main>
    <footer>Cookie banner</footer>
  </body>
</html>"#,
                )
            }),
        )
        .route(
            "/v1/chat/completions",
            post(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    json!({
                        "choices": [
                            {
                                "message": {
                                    "content": "{\"summary\":\"Fixture summary from mock LLM.\"}"
                                }
                            }
                        ]
                    })
                    .to_string(),
                )
            }),
        );

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("mock llm fixture server crashed");
    });

    base_url
}

/// Builds a tiny Atom feed fixture with a single entry.
fn atom_feed(feed_id: &str, entry_id: &str, title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>{title}</title>
  <link href="https://example.com/{feed_id}" />
  <updated>2026-03-09T00:00:00Z</updated>
  <id>tag:example.com,2026:{feed_id}</id>
  <entry>
    <title>{title}</title>
    <link href="https://example.com/{entry_id}" />
    <id>tag:example.com,2026:{entry_id}</id>
    <updated>2026-03-09T00:00:00Z</updated>
    <summary>{title} summary</summary>
  </entry>
</feed>"#
    )
}

fn llm_atom_feed(base_url: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
    <title>LLM Fixture Feed</title>
    <link href="{base_url}/feeds/llm" />
    <updated>2026-03-09T00:00:00Z</updated>
    <id>tag:example.com,2026:llm-feed</id>
    <entry>
        <title>LLM Fixture Entry</title>
        <link href="{base_url}/articles/llm-entry" />
        <id>tag:example.com,2026:llm-entry</id>
        <updated>2026-03-09T00:00:00Z</updated>
        <content type="html"><![CDATA[<p>Short teaser only.</p>]]></content>
    </entry>
</feed>"#
    )
}

/// Returns a free localhost TCP port for test server startup.
fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("failed to bind ephemeral port")
        .local_addr()
        .expect("failed to read local addr")
        .port()
}

/// Resolves the binary path produced by Cargo for this crate.
fn binary_path() -> PathBuf {
    PathBuf::from(assert_cmd::cargo::cargo_bin!("headless-rss"))
}

/// Encodes an HTTP Basic auth header value.
fn basic_auth_value(username: &str, password: &str) -> String {
    let encoded = BASE64.encode(format!("{username}:{password}"));
    format!("Basic {encoded}")
}
