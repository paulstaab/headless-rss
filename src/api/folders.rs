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
use crate::repo;

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

    let existing = repo::folder_name_exists(&state.pool, &input.name, None)
        .await
        .map_err(internal_error)?;
    if existing {
        return Err(folder_already_exists());
    }

    let folder_id = repo::create_folder(&state.pool, &input.name)
        .await
        .map_err(internal_error)?;
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
    if !repo::folder_exists(&state.pool, folder_id)
        .await
        .map_err(internal_error)?
    {
        return Err(folder_not_found());
    }

    let counts = repo::delete_folder_cascade(&state.pool, folder_id)
        .await
        .map_err(internal_error)?;

    tracing::info!(
        folder_id,
        deleted_articles = counts.deleted_articles,
        deleted_feeds = counts.deleted_feeds,
        deleted_folders = counts.deleted_folders,
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

    if !repo::folder_exists(&state.pool, folder_id)
        .await
        .map_err(internal_error)?
    {
        return Err(folder_not_found());
    }

    if repo::folder_name_exists(&state.pool, &input.name, Some(folder_id))
        .await
        .map_err(internal_error)?
    {
        return Err(folder_already_exists());
    }

    repo::rename_folder(&state.pool, folder_id, &input.name)
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
    if !repo::folder_exists(&state.pool, folder_id)
        .await
        .map_err(internal_error)?
    {
        return Err(folder_not_found());
    }

    repo::mark_folder_items_read(&state.pool, folder_id, input.newest_item_id)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::OK)
}

/// Resolves `null` and `0` to the internal root folder and validates explicit ids.
pub(super) async fn resolve_folder_id(pool: &SqlitePool, folder_id: Option<i64>) -> ApiResult<i64> {
    if folder_id.is_none() || folder_id == Some(0) {
        return repo::get_root_folder_id(pool).await.map_err(internal_error);
    }

    let requested_id = folder_id.unwrap_or_default();
    let exists = repo::folder_exists(pool, requested_id)
        .await
        .map_err(internal_error)?;
    if !exists {
        return Err(folder_not_found_with_id(requested_id));
    }

    Ok(requested_id)
}
