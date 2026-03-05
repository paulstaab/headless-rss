use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqlitePool};
use tower_http::cors::CorsLayer;

use crate::config::Config;

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
    let config_for_middleware = state.config.clone();

    let protected_v1_2 = Router::new()
        .route("/feeds", get(get_feeds))
        .route("/folders", get(get_folders))
        .route("/items", get(get_items))
        .route("/items/updated", get(get_updated_items))
        .route("/items/{item_id}/content", get(get_item_content))
        .route("/version", get(get_version))
        .route_layer(middleware::from_fn_with_state(
            config_for_middleware.clone(),
            require_basic_auth,
        ));

    let protected_v1_3 = Router::new()
        .route("/feeds", get(get_feeds))
        .route("/folders", get(get_folders))
        .route("/items", get(get_items))
        .route("/items/updated", get(get_updated_items))
        .route("/items/{item_id}/content", get(get_item_content))
        .route("/version", get(get_version))
        .route_layer(middleware::from_fn_with_state(
            config_for_middleware,
            require_basic_auth,
        ));

    Router::new()
        .route("/status", get(status))
        .nest("/index.php/apps/news/api/v1-2", protected_v1_2)
        .nest("/index.php/apps/news/api/v1-3", protected_v1_3)
        .with_state(state)
        .layer(CorsLayer::permissive())
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

async fn get_feeds(
    State(state): State<AppState>,
) -> Result<Json<FeedGetOut>, (StatusCode, Json<serde_json::Value>)> {
    let root_folder_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM folder WHERE is_root = 1 LIMIT 1")
            .fetch_optional(&state.pool)
            .await
            .map_err(internal_error)?;

    let rows = sqlx::query_as::<_, FeedRow>(
        "SELECT id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error FROM feed ORDER BY id",
    )
    .fetch_all(&state.pool)
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

    Ok(Json(FeedGetOut { feeds }))
}

async fn get_items(
    State(state): State<AppState>,
    Query(params): Query<ItemsQueryParams>,
) -> Result<Json<ItemGetOut>, (StatusCode, Json<serde_json::Value>)> {
    let rows = query_items(
        &state.pool,
        params.r#type,
        params.id,
        params.get_read,
        params.oldest_first,
        params.last_modified,
        params.offset,
        params.batch_size,
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
        params.r#type,
        params.id,
        true,
        false,
        params.last_modified,
        0,
        -1,
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
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "detail": "Item not found" })),
        ));
    };

    Ok(Json(ItemContentOut { content }))
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
    selection_type: i64,
    selection_id: i64,
    get_read: bool,
    oldest_first: bool,
    last_modified: i64,
    newest_item_id: i64,
    batch_size: i64,
) -> Result<Vec<ItemRow>, (StatusCode, Json<serde_json::Value>)> {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;
    use serde_json::Value;
    use sqlx::SqlitePool;
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

    #[tokio::test]
    async fn status_endpoint_returns_ok() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
            }),
        };

        let response = app(state)
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
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
            }),
        };

        let response = app(state)
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
    async fn protected_endpoints_require_auth_when_configured() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
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

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed, serde_json::json!({ "detail": "Not authenticated" }));
    }

    #[tokio::test]
    async fn items_endpoint_returns_items_with_body() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
            }),
        };

        let response = app(state)
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
    async fn updated_items_endpoint_filters_by_last_modified() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
            }),
        };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .uri(
                        "/index.php/apps/news/api/v1-2/items/updated?lastModified=199&type=0&id=10",
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
        assert_eq!(parsed["items"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn item_content_returns_404_when_missing() {
        let pool = setup_pool().await;
        let state = AppState {
            pool,
            config: Arc::new(Config {
                username: None,
                password: None,
                version: "dev".to_string(),
                db_path: "data/headless-rss.sqlite3".to_string(),
            }),
        };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .uri("/index.php/apps/news/api/v1-3/items/999999/content")
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
        assert_eq!(parsed, serde_json::json!({ "detail": "Item not found" }));
    }
}
