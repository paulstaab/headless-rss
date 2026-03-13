//! Feed handlers and feed-related article ingestion helpers.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::article_store;
use crate::config::Config;
use crate::content::{self, FeedContentState};
use crate::ssrf;

use super::AppState;
use super::errors::{
    ApiResult, feed_already_exists, feed_not_found_with_id, feed_parse_error,
    internal_anyhow_error, internal_error, ssrf_error,
};
use super::folders::resolve_folder_id;

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
pub(super) struct FeedOut {
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
pub(super) struct FeedGetOut {
    feeds: Vec<FeedOut>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FeedCreateIn {
    url: String,
    folder_id: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FeedCreateOut {
    feeds: Vec<FeedOut>,
    newest_item_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FeedMoveIn {
    pub(super) folder_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FeedRenameIn {
    pub(super) feed_title: String,
}

/// Lists configured feeds and maps the internal root folder to `null`.
pub(super) async fn get_feeds(State(state): State<AppState>) -> ApiResult<Json<FeedGetOut>> {
    let feeds = load_feeds(&state.pool).await?;
    Ok(Json(FeedGetOut { feeds }))
}

/// Adds a feed through the v1-2 API variant.
pub(super) async fn v1_2_add_feed(
    State(state): State<AppState>,
    Json(input): Json<FeedCreateIn>,
) -> ApiResult<Json<FeedCreateOut>> {
    add_feed(
        &state.pool,
        &state.feed_http_client,
        &state.article_http_client,
        &state.config,
        input,
    )
    .await
}

/// Adds a feed through the v1-3 API variant.
pub(super) async fn v1_3_add_feed(
    State(state): State<AppState>,
    Json(input): Json<FeedCreateIn>,
) -> ApiResult<Json<FeedCreateOut>> {
    add_feed(
        &state.pool,
        &state.feed_http_client,
        &state.article_http_client,
        &state.config,
        input,
    )
    .await
}

/// Deletes a feed and all associated articles.
pub(super) async fn delete_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
) -> ApiResult<StatusCode> {
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

/// Moves a feed through the v1-2 API variant.
pub(super) async fn v1_2_move_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedMoveIn>,
) -> ApiResult<StatusCode> {
    move_feed(&state.pool, feed_id, input.folder_id).await
}

/// Moves a feed through the v1-3 API variant.
pub(super) async fn v1_3_move_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedMoveIn>,
) -> ApiResult<StatusCode> {
    move_feed(&state.pool, feed_id, input.folder_id).await
}

/// Renames a feed through the v1-2 API variant.
pub(super) async fn v1_2_rename_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedRenameIn>,
) -> ApiResult<StatusCode> {
    rename_feed(&state.pool, feed_id, &input.feed_title).await
}

/// Renames a feed through the v1-3 API variant.
pub(super) async fn v1_3_rename_feed(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<FeedRenameIn>,
) -> ApiResult<StatusCode> {
    rename_feed(&state.pool, feed_id, &input.feed_title).await
}

/// Marks a feed's items as read through the v1-2 API variant.
pub(super) async fn v1_2_mark_feed_items_read(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<super::folders::MarkAllItemsReadIn>,
) -> ApiResult<StatusCode> {
    mark_feed_items_read(&state.pool, feed_id, input.newest_item_id).await
}

/// Marks a feed's items as read through the v1-3 API variant.
pub(super) async fn v1_3_mark_feed_items_read(
    State(state): State<AppState>,
    Path(feed_id): Path<i64>,
    Json(input): Json<super::folders::MarkAllItemsReadIn>,
) -> ApiResult<StatusCode> {
    mark_feed_items_read(&state.pool, feed_id, input.newest_item_id).await
}

async fn add_feed(
    pool: &SqlitePool,
    feed_http_client: &reqwest::Client,
    article_http_client: &reqwest::Client,
    config: &Config,
    input: FeedCreateIn,
) -> ApiResult<Json<FeedCreateOut>> {
    ssrf::validate_remote_url(&input.url, config.testing_mode)
        .await
        .map_err(ssrf_error)?;

    let folder_id = resolve_folder_id(pool, input.folder_id).await?;

    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE url = ? LIMIT 1")
        .bind(&input.url)
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;
    if existing.is_some() {
        return Err(feed_already_exists());
    }

    let response = feed_http_client
        .get(&input.url)
        .send()
        .await
        .map_err(feed_parse_error)?;
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

    let now_ts = article_store::unix_now();
    let title = parsed.title.map(|t| t.content);
    let link = parsed.links.first().map(|l| l.href.clone());
    let feed_title = title.clone();

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

    let content_state = content::maybe_refresh_feed_content_state(
        pool,
        article_http_client,
        config,
        feed_id,
        feed_title.as_deref(),
        FeedContentState {
            last_quality_check: None,
            use_extracted_fulltext: false,
            use_llm_summary: false,
        },
        &parsed.entries,
    )
    .await
    .map_err(internal_anyhow_error)?;

    for entry in parsed.entries.iter().take(50) {
        insert_article_from_entry(
            pool,
            article_http_client,
            config,
            feed_id,
            entry,
            content_state,
        )
        .await?;
    }

    let feeds = load_feeds(pool).await?;
    Ok(Json(FeedCreateOut {
        feeds,
        newest_item_id: feed_id,
    }))
}

async fn insert_article_from_entry(
    pool: &SqlitePool,
    article_http_client: &reqwest::Client,
    config: &Config,
    feed_id: i64,
    entry: &feed_rs::model::Entry,
    content_state: FeedContentState,
) -> ApiResult<()> {
    let Some(article) = article_store::article_record_from_feed_entry(
        article_http_client,
        config,
        feed_id,
        entry,
        content_state,
    )
    .await
    else {
        return Ok(());
    };

    let _ = article_store::insert_article_if_new(pool, article)
        .await
        .map_err(internal_error)?;

    Ok(())
}

async fn move_feed(pool: &SqlitePool, feed_id: i64, folder_id: Option<i64>) -> ApiResult<StatusCode> {
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

async fn rename_feed(pool: &SqlitePool, feed_id: i64, feed_title: &str) -> ApiResult<StatusCode> {
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
) -> ApiResult<StatusCode> {
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

/// Loads feeds in API response format.
pub(super) async fn load_feeds(pool: &SqlitePool) -> ApiResult<Vec<FeedOut>> {
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
