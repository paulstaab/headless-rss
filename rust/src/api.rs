use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{net::IpAddr, str::FromStr};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use feed_rs::model::Entry;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use tokio::net::lookup_host;
use tower_http::cors::CorsLayer;

use crate::config::Config;

mod v1_2;
mod v1_3;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
}

#[derive(Serialize)]
struct StatusOut {
    status: &'static str,
}

#[derive(Serialize)]
struct VersionOut {
    version: String,
}

#[derive(FromRow)]
struct FeedRow {
    id: i64,
    url: String,
    title: Option<String>,
    favicon_link: Option<String>,
    added: i64,
    next_update_time: Option<i64>,
    folder_id: i64,
    ordering: i64,
    link: Option<String>,
    pinned: bool,
    update_error_count: i64,
    last_update_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedOut {
    id: i64,
    url: String,
    title: Option<String>,
    favicon_link: Option<String>,
    added: i64,
    next_update_time: Option<i64>,
    folder_id: Option<i64>,
    ordering: i64,
    link: Option<String>,
    pinned: bool,
    update_error_count: i64,
    last_update_error: Option<String>,
}

#[derive(Serialize)]
struct FeedGetOut {
    feeds: Vec<FeedOut>,
}

#[derive(FromRow, Serialize)]
struct FolderOut {
    id: i64,
    name: String,
}

#[derive(Serialize)]
struct FolderGetOut {
    folders: Vec<FolderOut>,
}

#[derive(Deserialize)]
struct FolderCreateIn {
    name: String,
}

#[derive(Serialize)]
struct FolderCreateOut {
    folders: Vec<FolderOut>,
}

#[derive(Deserialize)]
struct FolderRenameIn {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedCreateIn {
    url: String,
    folder_id: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedCreateOut {
    feeds: Vec<FeedOut>,
    newest_item_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedMoveIn {
    folder_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedRenameIn {
    feed_title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemsQueryParams {
    #[serde(default = "default_batch_size")]
    batch_size: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default = "default_selection_type")]
    r#type: i64,
    #[serde(default)]
    id: i64,
    #[serde(default = "default_get_read")]
    get_read: bool,
    #[serde(default)]
    oldest_first: bool,
    #[serde(default)]
    last_modified: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatedItemsQueryParams {
    last_modified: i64,
    r#type: i64,
    id: i64,
}

#[derive(FromRow)]
struct ItemRow {
    id: i64,
    title: Option<String>,
    content: Option<String>,
    author: Option<String>,
    content_hash: Option<String>,
    enclosure_link: Option<String>,
    enclosure_mime: Option<String>,
    feed_id: i64,
    fingerprint: Option<String>,
    guid: String,
    guid_hash: String,
    last_modified: i64,
    media_description: Option<String>,
    media_thumbnail: Option<String>,
    pub_date: Option<i64>,
    rtl: bool,
    starred: bool,
    unread: bool,
    updated_date: Option<i64>,
    url: Option<String>,
    summary: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemOut {
    id: i64,
    title: Option<String>,
    author: Option<String>,
    body: Option<String>,
    content_hash: Option<String>,
    enclosure_link: Option<String>,
    enclosure_mime: Option<String>,
    feed_id: i64,
    fingerprint: Option<String>,
    guid: String,
    guid_hash: String,
    last_modified: i64,
    media_description: Option<String>,
    media_thumbnail: Option<String>,
    pub_date: Option<i64>,
    rtl: bool,
    starred: bool,
    unread: bool,
    updated_date: Option<i64>,
    url: Option<String>,
}

#[derive(Serialize)]
struct ItemGetOut {
    items: Vec<ItemOut>,
}

#[derive(Serialize)]
struct ItemContentOut {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ItemIdsV12In {
    items: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemIdsV13In {
    item_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuidItemIn {
    feed_id: i64,
    guid_hash: String,
}

#[derive(Deserialize)]
struct GuidItemsIn {
    items: Vec<GuidItemIn>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkAllItemsReadIn {
    newest_item_id: i64,
}

fn default_batch_size() -> i64 {
    -1
}

fn default_selection_type() -> i64 {
    1
}

fn default_get_read() -> bool {
    true
}

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

async fn log_user_interaction(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let started_at = Instant::now();
    let response = next.run(request).await;
    let duration_ms = started_at.elapsed().as_millis() as u64;

    tracing::debug!(
        method = %method,
        uri = %uri,
        status = response.status().as_u16(),
        duration_ms,
        "user interaction handled"
    );

    response
}

async fn status() -> Json<StatusOut> {
    Json(StatusOut { status: "ok" })
}

async fn get_version(State(state): State<AppState>) -> Json<VersionOut> {
    Json(VersionOut {
        version: state.config.version.clone(),
    })
}

async fn get_folders(
    State(state): State<AppState>,
) -> Result<Json<FolderGetOut>, (StatusCode, Json<serde_json::Value>)> {
    let rows =
        sqlx::query_as::<_, FolderOut>("SELECT id, name FROM folder WHERE is_root = 0 ORDER BY id")
            .fetch_all(&state.pool)
            .await
            .map_err(internal_error)?;

    Ok(Json(FolderGetOut { folders: rows }))
}

async fn create_folder(
    State(state): State<AppState>,
    Json(input): Json<FolderCreateIn>,
) -> Result<Json<FolderCreateOut>, (StatusCode, Json<serde_json::Value>)> {
    if input.name.is_empty() {
        return Err(folder_name_invalid());
    }

    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM folder WHERE name = ? LIMIT 1")
        .bind(&input.name)
        .fetch_optional(&state.pool)
        .await
        .map_err(internal_error)?;

    if existing.is_some() {
        return Err(folder_already_exists());
    }

    let result = sqlx::query("INSERT INTO folder (name, is_root) VALUES (?, 0)")
        .bind(&input.name)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?;

    let folder_id = result.last_insert_rowid();
    Ok(Json(FolderCreateOut {
        folders: vec![FolderOut {
            id: folder_id,
            name: input.name,
        }],
    }))
}

async fn delete_folder(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let folder_exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE id = ? LIMIT 1")
            .bind(folder_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;

    if folder_exists.is_none() {
        return Err(folder_not_found());
    }

    // Match Python behavior: deleting a folder removes its feeds and articles.
    let deleted_articles = sqlx::query(
        "DELETE FROM article WHERE feed_id IN (SELECT id FROM feed WHERE folder_id = ?)",
    )
    .bind(folder_id)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?
    .rows_affected();
    let deleted_feeds = sqlx::query("DELETE FROM feed WHERE folder_id = ?")
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?
        .rows_affected();
    let deleted_folders = sqlx::query("DELETE FROM folder WHERE id = ?")
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?
        .rows_affected();

    tracing::info!(
        folder_id,
        deleted_articles,
        deleted_feeds,
        deleted_folders,
        "folder cleanup completed"
    );

    Ok(StatusCode::OK)
}

async fn rename_folder(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
    Json(input): Json<FolderRenameIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if input.name.is_empty() {
        return Err(folder_name_invalid());
    }

    let folder_exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE id = ? LIMIT 1")
            .bind(folder_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;
    if folder_exists.is_none() {
        return Err(folder_not_found());
    }

    let duplicate: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE name = ? AND id != ? LIMIT 1")
            .bind(&input.name)
            .bind(folder_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;
    if duplicate.is_some() {
        return Err(folder_already_exists());
    }

    sqlx::query("UPDATE folder SET name = ? WHERE id = ?")
        .bind(&input.name)
        .bind(folder_id)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

async fn mark_folder_items_read(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let folder_exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE id = ? LIMIT 1")
            .bind(folder_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;
    if folder_exists.is_none() {
        return Err(folder_not_found());
    }

    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) \
         WHERE feed_id IN (SELECT id FROM feed WHERE folder_id = ?) AND id <= ?",
    )
    .bind(folder_id)
    .bind(input.newest_item_id)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

async fn get_feeds(
    State(state): State<AppState>,
) -> Result<Json<FeedGetOut>, (StatusCode, Json<serde_json::Value>)> {
    let feeds = load_feeds(&state.pool).await?;
    Ok(Json(FeedGetOut { feeds }))
}

async fn v1_2_add_feed(
    State(state): State<AppState>,
    Json(input): Json<FeedCreateIn>,
) -> Result<Json<FeedCreateOut>, (StatusCode, Json<serde_json::Value>)> {
    add_feed(&state.pool, &state.config, input).await
}

async fn v1_3_add_feed(
    State(state): State<AppState>,
    Json(input): Json<FeedCreateIn>,
) -> Result<Json<FeedCreateOut>, (StatusCode, Json<serde_json::Value>)> {
    add_feed(&state.pool, &state.config, input).await
}

async fn delete_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let feed_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE id = ? LIMIT 1")
        .bind(feed_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(internal_error)?;

    if feed_exists.is_none() {
        return Err(feed_not_found_with_id(feed_id));
    }

    let deleted_articles = sqlx::query("DELETE FROM article WHERE feed_id = ?")
        .bind(feed_id)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?
        .rows_affected();
    let deleted_feeds = sqlx::query("DELETE FROM feed WHERE id = ?")
        .bind(feed_id)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?
        .rows_affected();

    tracing::info!(
        feed_id,
        deleted_articles,
        deleted_feeds,
        "feed/article cleanup completed"
    );

    Ok(StatusCode::OK)
}

async fn v1_2_move_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedMoveIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    move_feed(&state.pool, feed_id, input.folder_id).await
}

async fn v1_3_move_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedMoveIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    move_feed(&state.pool, feed_id, input.folder_id).await
}

async fn v1_2_rename_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedRenameIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    rename_feed(&state.pool, feed_id, &input.feed_title).await
}

async fn v1_3_rename_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedRenameIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    rename_feed(&state.pool, feed_id, &input.feed_title).await
}

async fn v1_2_mark_feed_items_read(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_feed_items_read(&state.pool, feed_id, input.newest_item_id).await
}

async fn v1_3_mark_feed_items_read(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_feed_items_read(&state.pool, feed_id, input.newest_item_id).await
}

async fn add_feed(
    pool: &SqlitePool,
    config: &Config,
    input: FeedCreateIn,
) -> Result<Json<FeedCreateOut>, (StatusCode, Json<serde_json::Value>)> {
    validate_remote_url(&input.url, config.testing_mode).await?;

    let folder_id = resolve_folder_id(pool, input.folder_id).await?;

    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE url = ? LIMIT 1")
        .bind(&input.url)
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;
    if existing.is_some() {
        return Err(feed_already_exists());
    }

    let response = reqwest::get(&input.url).await.map_err(feed_parse_error)?;
    if !response.status().is_success() {
        return Err(feed_parse_error(format!(
            "Error parsing feed from `{}`: HTTP {}",
            input.url,
            response.status()
        )));
    }

    let bytes = response.bytes().await.map_err(feed_parse_error)?;
    let parsed = feed_rs::parser::parse(&bytes[..]).map_err(|err| {
        feed_parse_error(format!("Error parsing feed from `{}`: {err}", input.url))
    })?;

    let now_ts = unix_now();
    let title = parsed.title.map(|t| t.content);
    let link = parsed.links.first().map(|l| l.href.clone());

    let result = sqlx::query(
        "INSERT INTO feed (url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (?, ?, NULL, ?, ?, ?, 0, ?, 0, 0, NULL)",
    )
    .bind(&input.url)
    .bind(title)
    .bind(now_ts)
    .bind(now_ts + 86_400)
    .bind(folder_id)
    .bind(link)
    .execute(pool)
    .await
    .map_err(internal_error)?;
    let feed_id = result.last_insert_rowid();

    for entry in parsed.entries.iter().take(50) {
        insert_article_from_entry(pool, feed_id, entry).await?;
    }

    let feeds = load_feeds(pool).await?;
    Ok(Json(FeedCreateOut {
        feeds,
        newest_item_id: feed_id,
    }))
}

async fn validate_remote_url(
    url: &str,
    allow_localhost: bool,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let parsed = reqwest::Url::parse(url).map_err(|_| {
        ssrf_error("URL scheme '' is not allowed. Only http and https are permitted.")
    })?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(ssrf_error(format!(
            "URL scheme '{}' is not allowed. Only http and https are permitted.",
            parsed.scheme()
        )));
    }

    let Some(hostname) = parsed.host_str() else {
        return Err(ssrf_error("URL must have a valid hostname."));
    };

    if !allow_localhost && matches!(hostname, "localhost" | "127.0.0.1" | "::1") {
        return Err(ssrf_error("Access to localhost is not allowed."));
    }

    // If the hostname itself is an IP literal, validate directly.
    if let Ok(ip) = IpAddr::from_str(hostname) {
        validate_ip_address(ip, allow_localhost)?;
    }

    // Resolve DNS and validate each resolved IP. If resolution fails, let HTTP fetch decide.
    let lookup_port = parsed.port_or_known_default().unwrap_or(80);
    if let Ok(addrs) = lookup_host((hostname, lookup_port)).await {
        for addr in addrs {
            validate_ip_address(addr.ip(), allow_localhost)?;
        }
    }

    Ok(())
}

fn validate_ip_address(
    ip: IpAddr,
    allow_localhost: bool,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let is_private = match ip {
        IpAddr::V4(v4) => v4.is_private(),
        IpAddr::V6(v6) => v6.is_unique_local(),
    };
    let is_link_local = match ip {
        IpAddr::V4(v4) => v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_unicast_link_local(),
    };

    if !allow_localhost && ip.is_loopback() {
        return Err(ssrf_error(format!(
            "Access to loopback address {ip} is not allowed."
        )));
    }

    if is_private && !ip.is_loopback() {
        return Err(ssrf_error(format!(
            "Access to private address {ip} is not allowed."
        )));
    }

    if is_link_local {
        return Err(ssrf_error(format!(
            "Access to link-local address {ip} is not allowed."
        )));
    }

    if ip.is_unspecified() {
        return Err(ssrf_error(format!(
            "Access to unspecified address {ip} is not allowed."
        )));
    }

    if ip.is_multicast() {
        return Err(ssrf_error(format!(
            "Access to multicast address {ip} is not allowed."
        )));
    }

    if ip == IpAddr::from([169, 254, 169, 254]) {
        return Err(ssrf_error(
            "Access to cloud metadata service is not allowed.",
        ));
    }

    Ok(())
}

async fn insert_article_from_entry(
    pool: &SqlitePool,
    feed_id: i64,
    entry: &Entry,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let guid = if entry.id.is_empty() {
        entry
            .links
            .first()
            .map(|l| l.href.clone())
            .or_else(|| entry.title.as_ref().map(|t| t.content.clone()))
    } else {
        Some(entry.id.clone())
    };

    let Some(guid) = guid else {
        return Ok(());
    };

    let guid_hash = format!("{:x}", md5::compute(guid.as_bytes()));
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM article WHERE guid_hash = ? LIMIT 1")
            .bind(&guid_hash)
            .fetch_optional(pool)
            .await
            .map_err(internal_error)?;
    if existing.is_some() {
        return Ok(());
    }

    let content = entry
        .content
        .as_ref()
        .and_then(|content| content.body.clone());
    let summary = entry.summary.as_ref().map(|s| s.content.clone());
    let media_thumbnail = extract_first_image_url(content.as_deref().or(summary.as_deref()));
    let content_hash = content
        .as_ref()
        .map(|value| format!("{:x}", md5::compute(value.as_bytes())));
    let title = entry.title.as_ref().map(|t| t.content.clone());
    let url = entry.links.first().map(|l| l.href.clone());
    let author = entry.authors.first().map(|a| a.name.clone());
    let now_ts = unix_now();
    let updated = entry.updated.map(|dt| dt.timestamp()).unwrap_or(now_ts);
    let published = entry.published.map(|dt| dt.timestamp()).unwrap_or(updated);

    sqlx::query(
        "INSERT INTO article (title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (?, ?, ?, ?, NULL, NULL, ?, NULL, ?, ?, ?, NULL, ?, ?, 0, 0, 1, ?, ?, ?)",
    )
    .bind(title)
    .bind(content)
    .bind(author)
    .bind(content_hash)
    .bind(feed_id)
    .bind(guid)
    .bind(guid_hash)
    .bind(now_ts)
    .bind(media_thumbnail)
    .bind(published)
    .bind(updated)
    .bind(url)
    .bind(summary)
    .execute(pool)
    .await
    .map_err(internal_error)?;

    Ok(())
}

/// Extracts the first image source URL from HTML body content.
fn extract_first_image_url(html_content: Option<&str>) -> Option<String> {
    let html = html_content?;

    static IMG_SRC_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = IMG_SRC_REGEX.get_or_init(|| {
        Regex::new(r#"(?is)<img[^>]*\bsrc\s*=\s*[\"']([^\"']+)[\"']"#)
            .expect("valid image src regex")
    });

    regex
        .captures(html)
        .and_then(|captures| captures.get(1).map(|m| m.as_str().to_string()))
}

async fn move_feed(
    pool: &SqlitePool,
    feed_id: i64,
    folder_id: Option<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let feed_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE id = ? LIMIT 1")
        .bind(feed_id)
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;
    if feed_exists.is_none() {
        return Err(feed_not_found_with_id(feed_id));
    }

    let resolved_folder_id = resolve_folder_id(pool, folder_id).await?;
    sqlx::query("UPDATE feed SET folder_id = ? WHERE id = ?")
        .bind(resolved_folder_id)
        .bind(feed_id)
        .execute(pool)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

async fn rename_feed(
    pool: &SqlitePool,
    feed_id: i64,
    feed_title: &str,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let result = sqlx::query("UPDATE feed SET title = ? WHERE id = ?")
        .bind(feed_title)
        .bind(feed_id)
        .execute(pool)
        .await
        .map_err(internal_error)?;

    if result.rows_affected() == 0 {
        return Err(feed_not_found_with_id(feed_id));
    }
    Ok(StatusCode::OK)
}

async fn mark_feed_items_read(
    pool: &SqlitePool,
    feed_id: i64,
    newest_item_id: i64,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let feed_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE id = ? LIMIT 1")
        .bind(feed_id)
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;
    if feed_exists.is_none() {
        return Err(feed_not_found_with_id(feed_id));
    }

    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE feed_id = ? AND id <= ?",
    )
    .bind(feed_id)
    .bind(newest_item_id)
    .execute(pool)
    .await
    .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

async fn resolve_folder_id(
    pool: &SqlitePool,
    folder_id: Option<i64>,
) -> Result<i64, (StatusCode, Json<serde_json::Value>)> {
    if folder_id.is_none() || folder_id == Some(0) {
        return get_root_folder_id(pool).await;
    }

    let requested_id = folder_id.unwrap_or_default();
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM folder WHERE id = ? LIMIT 1")
        .bind(requested_id)
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;

    if exists.is_none() {
        return Err(folder_not_found_with_id(requested_id));
    }

    Ok(requested_id)
}

async fn get_root_folder_id(
    pool: &SqlitePool,
) -> Result<i64, (StatusCode, Json<serde_json::Value>)> {
    let id: Option<i64> = sqlx::query_scalar("SELECT id FROM folder WHERE is_root = 1 LIMIT 1")
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;

    if let Some(id) = id {
        return Ok(id);
    }

    let result = sqlx::query("INSERT INTO folder (name, is_root) VALUES ('', 1)")
        .execute(pool)
        .await
        .map_err(internal_error)?;
    Ok(result.last_insert_rowid())
}

async fn load_feeds(
    pool: &SqlitePool,
) -> Result<Vec<FeedOut>, (StatusCode, Json<serde_json::Value>)> {
    let root_folder_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE is_root = 1 LIMIT 1")
            .fetch_optional(pool)
            .await
            .map_err(internal_error)?;

    let rows = sqlx::query_as::<_, FeedRow>(
        "SELECT id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error FROM feed ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(internal_error)?;

    let feeds = rows
        .into_iter()
        .map(|feed| FeedOut {
            id: feed.id,
            url: feed.url,
            title: feed.title,
            favicon_link: feed.favicon_link,
            added: feed.added,
            next_update_time: feed.next_update_time,
            folder_id: match root_folder_id {
                Some(root_id) if root_id == feed.folder_id => None,
                _ => Some(feed.folder_id),
            },
            ordering: feed.ordering,
            link: feed.link,
            pinned: feed.pinned,
            update_error_count: feed.update_error_count,
            last_update_error: feed.last_update_error,
        })
        .collect();

    Ok(feeds)
}

async fn get_items(
    State(state): State<AppState>,
    Query(params): Query<ItemsQueryParams>,
) -> Result<Json<ItemGetOut>, (StatusCode, Json<serde_json::Value>)> {
    let rows = query_items(
        &state.pool,
        QueryItemsInput {
            selection_type: params.r#type,
            selection_id: params.id,
            get_read: params.get_read,
            oldest_first: params.oldest_first,
            last_modified: params.last_modified,
            newest_item_id: params.offset,
            batch_size: params.batch_size,
        },
    )
    .await?;

    Ok(Json(ItemGetOut {
        items: rows.into_iter().map(item_row_to_out).collect(),
    }))
}

async fn get_updated_items(
    State(state): State<AppState>,
    Query(params): Query<UpdatedItemsQueryParams>,
) -> Result<Json<ItemGetOut>, (StatusCode, Json<serde_json::Value>)> {
    let rows = query_items(
        &state.pool,
        QueryItemsInput {
            selection_type: params.r#type,
            selection_id: params.id,
            get_read: true,
            oldest_first: false,
            last_modified: params.last_modified,
            newest_item_id: 0,
            batch_size: -1,
        },
    )
    .await?;

    Ok(Json(ItemGetOut {
        items: rows.into_iter().map(item_row_to_out).collect(),
    }))
}

async fn get_item_content(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<Json<ItemContentOut>, (StatusCode, Json<serde_json::Value>)> {
    let content: Option<Option<String>> =
        sqlx::query_scalar("SELECT content FROM article WHERE id = ?")
            .bind(item_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;

    let Some(content) = content else {
        return Err(item_not_found());
    };

    Ok(Json(ItemContentOut { content }))
}

async fn v1_2_mark_item_as_read(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state(&state.pool, &[item_id], false).await
}

async fn v1_2_mark_multiple_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV12In>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state_allow_empty(&state.pool, &input.items, false).await
}

async fn v1_2_mark_item_as_unread(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state(&state.pool, &[item_id], true).await
}

async fn v1_2_mark_multiple_items_as_unread(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV12In>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state_allow_empty(&state.pool, &input.items, true).await
}

async fn v1_2_mark_item_as_starred(
    State(state): State<AppState>,
    Path((feed_id, guid_hash)): Path<(i64, String)>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let article_id = get_article_id_by_guid_hash(&state.pool, feed_id, &guid_hash).await?;
    mark_items_star_state(&state.pool, &[article_id], true).await
}

async fn v1_2_mark_multiple_items_as_starred(
    State(state): State<AppState>,
    Json(input): Json<GuidItemsIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let mut ids = Vec::with_capacity(input.items.len());
    for item in input.items {
        ids.push(get_article_id_by_guid_hash(&state.pool, item.feed_id, &item.guid_hash).await?);
    }
    mark_items_star_state_allow_empty(&state.pool, &ids, true).await
}

async fn v1_2_mark_item_as_unstarred(
    State(state): State<AppState>,
    Path((feed_id, guid_hash)): Path<(i64, String)>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let article_id = get_article_id_by_guid_hash(&state.pool, feed_id, &guid_hash).await?;
    mark_items_star_state(&state.pool, &[article_id], false).await
}

async fn v1_2_mark_multiple_items_as_unstarred(
    State(state): State<AppState>,
    Json(input): Json<GuidItemsIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let mut ids = Vec::with_capacity(input.items.len());
    for item in input.items {
        ids.push(get_article_id_by_guid_hash(&state.pool, item.feed_id, &item.guid_hash).await?);
    }
    mark_items_star_state_allow_empty(&state.pool, &ids, false).await
}

async fn v1_2_mark_all_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_all_items_read(&state.pool, input.newest_item_id).await
}

async fn v1_3_mark_item_as_read(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state(&state.pool, &[item_id], false).await
}

async fn v1_3_mark_multiple_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state_allow_empty(&state.pool, &input.item_ids, false).await
}

async fn v1_3_mark_item_as_unread(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state(&state.pool, &[item_id], true).await
}

async fn v1_3_mark_multiple_items_as_unread(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_read_state_allow_empty(&state.pool, &input.item_ids, true).await
}

async fn v1_3_mark_item_as_starred(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_star_state(&state.pool, &[item_id], true).await
}

async fn v1_3_mark_multiple_items_as_starred(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_star_state_allow_empty(&state.pool, &input.item_ids, true).await
}

async fn v1_3_mark_item_as_unstarred(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_star_state(&state.pool, &[item_id], false).await
}

async fn v1_3_mark_multiple_items_as_unstarred(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_items_star_state_allow_empty(&state.pool, &input.item_ids, false).await
}

async fn v1_3_mark_all_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    mark_all_items_read(&state.pool, input.newest_item_id).await
}

fn item_row_to_out(item: ItemRow) -> ItemOut {
    ItemOut {
        id: item.id,
        title: item.title,
        author: item.author,
        body: item.summary.or(item.content),
        content_hash: item.content_hash,
        enclosure_link: item.enclosure_link,
        enclosure_mime: item.enclosure_mime,
        feed_id: item.feed_id,
        fingerprint: item.fingerprint,
        guid: item.guid,
        guid_hash: item.guid_hash,
        last_modified: item.last_modified,
        media_description: item.media_description,
        media_thumbnail: item.media_thumbnail,
        pub_date: item.pub_date,
        rtl: item.rtl,
        starred: item.starred,
        unread: item.unread,
        updated_date: item.updated_date,
        url: item.url,
    }
}

async fn query_items(
    pool: &SqlitePool,
    input: QueryItemsInput,
) -> Result<Vec<ItemRow>, (StatusCode, Json<serde_json::Value>)> {
    let QueryItemsInput {
        selection_type,
        selection_id,
        get_read,
        oldest_first,
        last_modified,
        newest_item_id,
        batch_size,
    } = input;

    let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT article.id, article.title, article.content, article.author, article.content_hash, article.enclosure_link, article.enclosure_mime, article.feed_id, article.fingerprint, article.guid, article.guid_hash, article.last_modified, article.media_description, article.media_thumbnail, article.pub_date, article.rtl, article.starred, article.unread, article.updated_date, article.url, article.summary FROM article",
    );

    if selection_type == 1 {
        qb.push(" JOIN feed ON feed.id = article.feed_id");
    }

    qb.push(" WHERE 1=1");

    match selection_type {
        0 => {
            qb.push(" AND article.feed_id = ");
            qb.push_bind(selection_id);
        }
        1 => {
            qb.push(" AND feed.folder_id = ");
            qb.push_bind(selection_id);
        }
        2 => {
            qb.push(" AND article.starred = 1");
        }
        3 => {}
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "detail": "Invalid item selection type" })),
            ));
        }
    }

    if !get_read {
        qb.push(" AND article.unread = 1");
    }

    if newest_item_id > 0 {
        qb.push(" AND article.id <= ");
        qb.push_bind(newest_item_id);
    }

    if last_modified > 0 {
        qb.push(" AND article.last_modified >= ");
        qb.push_bind(last_modified);
    }

    qb.push(" ORDER BY article.id ");
    if oldest_first {
        qb.push("ASC");
    } else {
        qb.push("DESC");
    }

    if batch_size > 0 {
        qb.push(" LIMIT ");
        qb.push_bind(batch_size);
    }

    qb.build_query_as::<ItemRow>()
        .fetch_all(pool)
        .await
        .map_err(internal_error)
}

struct QueryItemsInput {
    selection_type: i64,
    selection_id: i64,
    get_read: bool,
    oldest_first: bool,
    last_modified: i64,
    newest_item_id: i64,
    batch_size: i64,
}

async fn get_article_id_by_guid_hash(
    pool: &SqlitePool,
    feed_id: i64,
    guid_hash: &str,
) -> Result<i64, (StatusCode, Json<serde_json::Value>)> {
    let article_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM article WHERE feed_id = ? AND guid_hash = ? LIMIT 1")
            .bind(feed_id)
            .bind(guid_hash)
            .fetch_optional(pool)
            .await
            .map_err(internal_error)?;

    article_id.ok_or_else(item_not_found)
}

async fn mark_items_read_state(
    pool: &SqlitePool,
    item_ids: &[i64],
    unread: bool,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if item_ids.is_empty() {
        return Ok(StatusCode::OK);
    }

    let mut check_qb: QueryBuilder<'_, Sqlite> =
        QueryBuilder::new("SELECT COUNT(*) FROM article WHERE id IN (");
    {
        let mut separated = check_qb.separated(", ");
        for id in item_ids {
            separated.push_bind(*id);
        }
    }
    check_qb.push(")");

    let existing_count: i64 = check_qb
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(internal_error)?;
    if existing_count == 0 {
        return Err(item_not_found());
    }

    mark_items_read_state_allow_empty(pool, item_ids, unread).await
}

async fn mark_items_read_state_allow_empty(
    pool: &SqlitePool,
    item_ids: &[i64],
    unread: bool,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if item_ids.is_empty() {
        return Ok(StatusCode::OK);
    }

    let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("UPDATE article SET unread = ");
    qb.push_bind(unread);
    qb.push(", last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE id IN (");
    {
        let mut separated = qb.separated(", ");
        for id in item_ids {
            separated.push_bind(*id);
        }
    }
    qb.push(")");

    qb.build().execute(pool).await.map_err(internal_error)?;
    Ok(StatusCode::OK)
}

async fn mark_items_star_state(
    pool: &SqlitePool,
    item_ids: &[i64],
    starred: bool,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if item_ids.is_empty() {
        return Ok(StatusCode::OK);
    }

    let mut check_qb: QueryBuilder<'_, Sqlite> =
        QueryBuilder::new("SELECT COUNT(*) FROM article WHERE id IN (");
    {
        let mut separated = check_qb.separated(", ");
        for id in item_ids {
            separated.push_bind(*id);
        }
    }
    check_qb.push(")");

    let existing_count: i64 = check_qb
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(internal_error)?;
    if existing_count == 0 {
        return Err(item_not_found());
    }

    mark_items_star_state_allow_empty(pool, item_ids, starred).await
}

async fn mark_items_star_state_allow_empty(
    pool: &SqlitePool,
    item_ids: &[i64],
    starred: bool,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if item_ids.is_empty() {
        return Ok(StatusCode::OK);
    }

    let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("UPDATE article SET starred = ");
    qb.push_bind(starred);
    qb.push(", last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE id IN (");
    {
        let mut separated = qb.separated(", ");
        for id in item_ids {
            separated.push_bind(*id);
        }
    }
    qb.push(")");

    qb.build().execute(pool).await.map_err(internal_error)?;
    Ok(StatusCode::OK)
}

async fn mark_all_items_read(
    pool: &SqlitePool,
    newest_item_id: i64,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE id <= ?",
    )
    .bind(newest_item_id)
    .execute(pool)
    .await
    .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn feed_already_exists() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "detail": "Feed already exists" })),
    )
}

fn feed_parse_error<E: std::fmt::Display>(error: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({ "detail": error.to_string() })),
    )
}

fn ssrf_error<E: std::fmt::Display>(error: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "detail": error.to_string() })),
    )
}

fn feed_not_found_with_id(feed_id: i64) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "detail": format!("Feed {feed_id} not found") })),
    )
}

fn folder_not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "detail": "Folder not found" })),
    )
}

fn folder_not_found_with_id(folder_id: i64) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({
            "detail": format!("Folder with ID {folder_id} does not exist"),
        })),
    )
}

fn folder_already_exists() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::CONFLICT,
        Json(serde_json::json!({ "detail": "Folder already exists" })),
    )
}

fn folder_name_invalid() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(serde_json::json!({ "detail": "Folder name is invalid" })),
    )
}

async fn require_basic_auth(
    State(config): State<Arc<Config>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !config.auth_enabled() {
        return next.run(request).await;
    }

    let Some(auth_header) = request.headers().get(header::AUTHORIZATION) else {
        return unauthorized("Not authenticated");
    };

    let Ok(auth_header) = auth_header.to_str() else {
        return unauthorized("Invalid authentication credentials");
    };

    if !auth_header.starts_with("Basic ") {
        return unauthorized("Invalid authentication credentials");
    }

    let encoded = &auth_header[6..];
    let Ok(decoded_bytes) = BASE64.decode(encoded) else {
        return unauthorized("Invalid authentication credentials");
    };

    let Ok(decoded) = String::from_utf8(decoded_bytes) else {
        return unauthorized("Invalid authentication credentials");
    };

    let Some((username, password)) = decoded.split_once(':') else {
        return unauthorized("Invalid authentication credentials");
    };

    let expected_username = config.username.as_deref().unwrap_or_default();
    let expected_password = config.password.as_deref().unwrap_or_default();

    if username != expected_username || password != expected_password {
        return unauthorized("Invalid authentication credentials");
    }

    next.run(request).await
}

fn unauthorized(message: &'static str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Basic")],
        Json(serde_json::json!({ "detail": message })),
    )
        .into_response()
}

fn internal_error(error: sqlx::Error) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!(?error, "database operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "detail": "Internal server error" })),
    )
}

fn item_not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "detail": "Item not found" })),
    )
}

#[cfg(test)]
mod tests {
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
            "CREATE TABLE feed (id INTEGER PRIMARY KEY NOT NULL, url VARCHAR NOT NULL UNIQUE, title VARCHAR, favicon_link VARCHAR, added INTEGER NOT NULL, next_update_time INTEGER, folder_id INTEGER NOT NULL, ordering INTEGER NOT NULL, link VARCHAR, pinned BOOLEAN NOT NULL, update_error_count INTEGER NOT NULL, last_update_error VARCHAR)",
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error) VALUES (10, 'https://example.com/rss', 'Example Feed', NULL, 123, NULL, 1, 0, 'https://example.com', 0, 0, NULL)")
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
                testing_mode: true,
            }),
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
                testing_mode: true,
            }),
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
                testing_mode: false,
            }),
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
                testing_mode: true,
            }),
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
                testing_mode: true,
            }),
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
}
