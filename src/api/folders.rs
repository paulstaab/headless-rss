//! Folder handlers and folder-related database helpers.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use super::AppState;
use super::errors::{
    ApiResult, folder_already_exists, folder_name_invalid, folder_not_found,
    folder_not_found_with_id, internal_error,
};

#[derive(FromRow, Serialize)]
pub(super) struct FolderOut {
    id: i64,
    name: String,
}

#[derive(Serialize)]
pub(super) struct FolderGetOut {
    folders: Vec<FolderOut>,
}

#[derive(Deserialize)]
pub(super) struct FolderCreateIn {
    name: String,
}

#[derive(Serialize)]
pub(super) struct FolderCreateOut {
    folders: Vec<FolderOut>,
}

#[derive(Deserialize)]
pub(super) struct FolderRenameIn {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MarkAllItemsReadIn {
    pub(super) newest_item_id: i64,
}

/// Lists user-visible folders, excluding the internal root folder.
pub(super) async fn get_folders(State(state): State<AppState>) -> ApiResult<Json<FolderGetOut>> {
    let rows =
        sqlx::query_as::<_, FolderOut>("SELECT id, name FROM folder WHERE is_root = 0 ORDER BY id")
            .fetch_all(&state.pool)
            .await
            .map_err(internal_error)?;

    Ok(Json(FolderGetOut { folders: rows }))
}

/// Creates a new non-root folder.
pub(super) async fn create_folder(
    State(state): State<AppState>,
    Json(input): Json<FolderCreateIn>,
) -> ApiResult<Json<FolderCreateOut>> {
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

/// Deletes a folder and all feeds/articles contained within it.
pub(super) async fn delete_folder(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
) -> ApiResult<StatusCode> {
    let folder_exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE id = ? LIMIT 1")
            .bind(folder_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;

    if folder_exists.is_none() {
        return Err(folder_not_found());
    }

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

/// Renames an existing folder.
pub(super) async fn rename_folder(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
    Json(input): Json<FolderRenameIn>,
) -> ApiResult<StatusCode> {
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

/// Marks all folder items up to the requested boundary as read.
pub(super) async fn mark_folder_items_read(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> ApiResult<StatusCode> {
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

/// Resolves `null` and `0` to the internal root folder and validates explicit ids.
pub(super) async fn resolve_folder_id(pool: &SqlitePool, folder_id: Option<i64>) -> ApiResult<i64> {
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

/// Returns the root folder id, creating it when needed for a fresh database.
pub(super) async fn get_root_folder_id(pool: &SqlitePool) -> ApiResult<i64> {
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
