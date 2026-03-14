//! Item query and mutation handlers shared by both Nextcloud API versions.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};

use super::AppState;
use super::errors::{ApiResult, internal_error, item_not_found};
use super::folders::MarkAllItemsReadIn;

// Maximum number of GUID items allowed in a single request to prevent
// unbounded memory allocation and database load from user input.
const MAX_GUID_ITEMS_PER_REQUEST: usize = 10_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ItemsQueryParams {
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
pub(super) struct UpdatedItemsQueryParams {
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
pub(super) struct ItemGetOut {
    items: Vec<ItemOut>,
}

#[derive(Serialize)]
pub(super) struct ItemContentOut {
    content: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ItemIdsV12In {
    items: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ItemIdsV13In {
    item_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GuidItemIn {
    feed_id: i64,
    guid_hash: String,
}

#[derive(Deserialize)]
pub(super) struct GuidItemsIn {
    items: Vec<GuidItemIn>,
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

/// Lists items according to Nextcloud News query semantics.
pub(super) async fn get_items(
    State(state): State<AppState>,
    Query(params): Query<ItemsQueryParams>,
) -> ApiResult<Json<ItemGetOut>> {
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

/// Returns incrementally updated items.
pub(super) async fn get_updated_items(
    State(state): State<AppState>,
    Query(params): Query<UpdatedItemsQueryParams>,
) -> ApiResult<Json<ItemGetOut>> {
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

/// Returns the full stored content of a single item.
pub(super) async fn get_item_content(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<Json<ItemContentOut>> {
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

/// Marks a single v1-2 item as read.
pub(super) async fn v1_2_mark_item_as_read(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &[item_id],
        ItemMutation::unread(false),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-2 items as read.
pub(super) async fn v1_2_mark_multiple_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV12In>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &input.items,
        ItemMutation::unread(false),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks a single v1-2 item as unread.
pub(super) async fn v1_2_mark_item_as_unread(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &[item_id],
        ItemMutation::unread(true),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-2 items as unread.
pub(super) async fn v1_2_mark_multiple_items_as_unread(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV12In>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &input.items,
        ItemMutation::unread(true),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks a single v1-2 item as starred by guid hash.
pub(super) async fn v1_2_mark_item_as_starred(
    State(state): State<AppState>,
    Path((feed_id, guid_hash)): Path<(i64, String)>,
) -> ApiResult<StatusCode> {
    let article_id = get_article_id_by_guid_hash(&state.pool, feed_id, &guid_hash).await?;
    mark_item_ids(
        &state.pool,
        &[article_id],
        ItemMutation::starred(true),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-2 items as starred by guid hash.
pub(super) async fn v1_2_mark_multiple_items_as_starred(
    State(state): State<AppState>,
    Json(input): Json<GuidItemsIn>,
) -> ApiResult<StatusCode> {
    let ids = resolve_guid_item_ids(&state.pool, input.items).await?;
    mark_item_ids(
        &state.pool,
        &ids,
        ItemMutation::starred(true),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks a single v1-2 item as unstarred by guid hash.
pub(super) async fn v1_2_mark_item_as_unstarred(
    State(state): State<AppState>,
    Path((feed_id, guid_hash)): Path<(i64, String)>,
) -> ApiResult<StatusCode> {
    let article_id = get_article_id_by_guid_hash(&state.pool, feed_id, &guid_hash).await?;
    mark_item_ids(
        &state.pool,
        &[article_id],
        ItemMutation::starred(false),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-2 items as unstarred by guid hash.
pub(super) async fn v1_2_mark_multiple_items_as_unstarred(
    State(state): State<AppState>,
    Json(input): Json<GuidItemsIn>,
) -> ApiResult<StatusCode> {
    let ids = resolve_guid_item_ids(&state.pool, input.items).await?;
    mark_item_ids(
        &state.pool,
        &ids,
        ItemMutation::starred(false),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks all items as read through the v1-2 API variant.
pub(super) async fn v1_2_mark_all_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> ApiResult<StatusCode> {
    mark_all_items_read(&state.pool, input.newest_item_id).await
}

/// Marks a single v1-3 item as read.
pub(super) async fn v1_3_mark_item_as_read(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &[item_id],
        ItemMutation::unread(false),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-3 items as read.
pub(super) async fn v1_3_mark_multiple_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &input.item_ids,
        ItemMutation::unread(false),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks a single v1-3 item as unread.
pub(super) async fn v1_3_mark_item_as_unread(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &[item_id],
        ItemMutation::unread(true),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-3 items as unread.
pub(super) async fn v1_3_mark_multiple_items_as_unread(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &input.item_ids,
        ItemMutation::unread(true),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks a single v1-3 item as starred.
pub(super) async fn v1_3_mark_item_as_starred(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &[item_id],
        ItemMutation::starred(true),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-3 items as starred.
pub(super) async fn v1_3_mark_multiple_items_as_starred(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &input.item_ids,
        ItemMutation::starred(true),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks a single v1-3 item as unstarred.
pub(super) async fn v1_3_mark_item_as_unstarred(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &[item_id],
        ItemMutation::starred(false),
        MissingBehavior::RequireExisting,
    )
    .await
}

/// Marks multiple v1-3 items as unstarred.
pub(super) async fn v1_3_mark_multiple_items_as_unstarred(
    State(state): State<AppState>,
    Json(input): Json<ItemIdsV13In>,
) -> ApiResult<StatusCode> {
    mark_item_ids(
        &state.pool,
        &input.item_ids,
        ItemMutation::starred(false),
        MissingBehavior::AllowEmptyInput,
    )
    .await
}

/// Marks all items as read through the v1-3 API variant.
pub(super) async fn v1_3_mark_all_items_as_read(
    State(state): State<AppState>,
    Json(input): Json<MarkAllItemsReadIn>,
) -> ApiResult<StatusCode> {
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

async fn query_items(pool: &SqlitePool, input: QueryItemsInput) -> ApiResult<Vec<ItemRow>> {
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
) -> ApiResult<i64> {
    let article_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM article WHERE feed_id = ? AND guid_hash = ? LIMIT 1")
            .bind(feed_id)
            .bind(guid_hash)
            .fetch_optional(pool)
            .await
            .map_err(internal_error)?;

    article_id.ok_or_else(item_not_found)
}

async fn resolve_guid_item_ids(pool: &SqlitePool, items: Vec<GuidItemIn>) -> ApiResult<Vec<i64>> {
    let len = items.len();
    // Guard against unbounded allocations and excessive work from user-controlled input.
    if len > MAX_GUID_ITEMS_PER_REQUEST {
        return Err(internal_error(
            format!(
                "too many items in request: {} (max {})",
                len, MAX_GUID_ITEMS_PER_REQUEST
            )
            .into(),
        ));
    }

    let mut ids = Vec::with_capacity(len);
    for item in items {
        ids.push(get_article_id_by_guid_hash(pool, item.feed_id, &item.guid_hash).await?);
    }
    Ok(ids)
}

enum MissingBehavior {
    RequireExisting,
    AllowEmptyInput,
}

enum MutationField {
    Unread,
    Starred,
}

struct ItemMutation {
    field: MutationField,
    value: bool,
}

impl ItemMutation {
    fn unread(unread: bool) -> Self {
        Self {
            field: MutationField::Unread,
            value: unread,
        }
    }

    fn starred(starred: bool) -> Self {
        Self {
            field: MutationField::Starred,
            value: starred,
        }
    }

    fn column_name(&self) -> &'static str {
        match self.field {
            MutationField::Unread => "unread",
            MutationField::Starred => "starred",
        }
    }
}

async fn mark_item_ids(
    pool: &SqlitePool,
    item_ids: &[i64],
    mutation: ItemMutation,
    missing_behavior: MissingBehavior,
) -> ApiResult<StatusCode> {
    if item_ids.is_empty() {
        return Ok(StatusCode::OK);
    }

    if matches!(missing_behavior, MissingBehavior::RequireExisting) {
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
    }

    let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("UPDATE article SET ");
    qb.push(mutation.column_name());
    qb.push(" = ");
    qb.push_bind(mutation.value);
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

async fn mark_all_items_read(pool: &SqlitePool, newest_item_id: i64) -> ApiResult<StatusCode> {
    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE id <= ?",
    )
    .bind(newest_item_id)
    .execute(pool)
    .await
    .map_err(internal_error)?;

    Ok(StatusCode::OK)
}
