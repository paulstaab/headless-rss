//! Shared persistence helpers for folders, feeds, and articles.

use sqlx::{QueryBuilder, Sqlite, SqlitePool};

use crate::article_store::ArticleRecord;

/// Row-deletion counts returned by cascading feed cleanup operations.
pub struct FeedDeleteCounts {
    /// Number of article rows deleted before the feed row was removed.
    pub deleted_articles: u64,
    /// Number of feed rows removed for the requested identifier.
    pub deleted_feeds: u64,
}

/// Row-deletion counts returned by cascading folder cleanup operations.
pub struct FolderDeleteCounts {
    /// Number of article rows deleted from feeds that belonged to the folder.
    pub deleted_articles: u64,
    /// Number of feed rows deleted from the folder.
    pub deleted_feeds: u64,
    /// Number of folder rows deleted for the requested identifier.
    pub deleted_folders: u64,
}

/// Logical article flag fields supported by bulk mutations.
#[derive(Clone, Copy)]
pub enum ArticleFlag {
    /// Read/unread state used by item read tracking.
    Unread,
    /// Starred state used by item bookmarking.
    Starred,
}

impl ArticleFlag {
    fn column_name(self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Starred => "starred",
        }
    }
}

/// Returns whether a folder with the given identifier exists.
pub async fn folder_exists(pool: &SqlitePool, folder_id: i64) -> Result<bool, sqlx::Error> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM folder WHERE id = ? LIMIT 1")
        .bind(folder_id)
        .fetch_optional(pool)
        .await?;

    Ok(existing.is_some())
}

/// Returns whether a folder name already exists, optionally excluding one folder id.
pub async fn folder_name_exists(
    pool: &SqlitePool,
    name: &str,
    exclude_id: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let existing: Option<i64> = match exclude_id {
        Some(folder_id) => {
            sqlx::query_scalar("SELECT id FROM folder WHERE name = ? AND id != ? LIMIT 1")
                .bind(name)
                .bind(folder_id)
                .fetch_optional(pool)
                .await?
        }
        None => {
            sqlx::query_scalar("SELECT id FROM folder WHERE name = ? LIMIT 1")
                .bind(name)
                .fetch_optional(pool)
                .await?
        }
    };

    Ok(existing.is_some())
}

/// Inserts a non-root folder and returns the new identifier.
pub async fn create_folder(pool: &SqlitePool, name: &str) -> Result<i64, sqlx::Error> {
    let result = sqlx::query("INSERT INTO folder (name, is_root) VALUES (?, 0)")
        .bind(name)
        .execute(pool)
        .await?;

    Ok(result.last_insert_rowid())
}

/// Updates a folder name.
pub async fn rename_folder(
    pool: &SqlitePool,
    folder_id: i64,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE folder SET name = ? WHERE id = ?")
        .bind(name)
        .bind(folder_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Deletes a folder together with all feeds and articles stored under it.
pub async fn delete_folder_cascade(
    pool: &SqlitePool,
    folder_id: i64,
) -> Result<FolderDeleteCounts, sqlx::Error> {
    let deleted_articles = sqlx::query(
        "DELETE FROM article WHERE feed_id IN (SELECT id FROM feed WHERE folder_id = ?)",
    )
    .bind(folder_id)
    .execute(pool)
    .await?
    .rows_affected();
    let deleted_feeds = sqlx::query("DELETE FROM feed WHERE folder_id = ?")
        .bind(folder_id)
        .execute(pool)
        .await?
        .rows_affected();
    let deleted_folders = sqlx::query("DELETE FROM folder WHERE id = ?")
        .bind(folder_id)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(FolderDeleteCounts {
        deleted_articles,
        deleted_feeds,
        deleted_folders,
    })
}

/// Marks all items in a folder as read up to the provided newest item boundary.
pub async fn mark_folder_items_read(
    pool: &SqlitePool,
    folder_id: i64,
    newest_item_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) \
         WHERE feed_id IN (SELECT id FROM feed WHERE folder_id = ?) AND id <= ?",
    )
    .bind(folder_id)
    .bind(newest_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns the root folder id, creating it for fresh databases when necessary.
pub async fn get_root_folder_id(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    let id: Option<i64> = sqlx::query_scalar("SELECT id FROM folder WHERE is_root = 1 LIMIT 1")
        .fetch_optional(pool)
        .await?;

    if let Some(id) = id {
        return Ok(id);
    }

    let result = sqlx::query("INSERT INTO folder (name, is_root) VALUES ('', 1)")
        .execute(pool)
        .await?;
    Ok(result.last_insert_rowid())
}

/// Returns whether a feed with the given identifier exists.
pub async fn feed_exists(pool: &SqlitePool, feed_id: i64) -> Result<bool, sqlx::Error> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE id = ? LIMIT 1")
        .bind(feed_id)
        .fetch_optional(pool)
        .await?;

    Ok(existing.is_some())
}

/// Returns whether a feed URL already exists.
pub async fn feed_exists_by_url(pool: &SqlitePool, url: &str) -> Result<bool, sqlx::Error> {
    let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE url = ? LIMIT 1")
        .bind(url)
        .fetch_optional(pool)
        .await?;

    Ok(existing.is_some())
}

/// Updates the folder assignment for a feed.
pub async fn move_feed(pool: &SqlitePool, feed_id: i64, folder_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE feed SET folder_id = ? WHERE id = ?")
        .bind(folder_id)
        .bind(feed_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Updates the display title for a feed and returns the affected row count.
pub async fn rename_feed(pool: &SqlitePool, feed_id: i64, title: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("UPDATE feed SET title = ? WHERE id = ?")
        .bind(title)
        .bind(feed_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Deletes a feed and its articles.
pub async fn delete_feed_cascade(
    pool: &SqlitePool,
    feed_id: i64,
) -> Result<FeedDeleteCounts, sqlx::Error> {
    let deleted_articles = sqlx::query("DELETE FROM article WHERE feed_id = ?")
        .bind(feed_id)
        .execute(pool)
        .await?
        .rows_affected();
    let deleted_feeds = sqlx::query("DELETE FROM feed WHERE id = ?")
        .bind(feed_id)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(FeedDeleteCounts {
        deleted_articles,
        deleted_feeds,
    })
}

/// Marks all items in a feed as read up to the provided newest item boundary.
pub async fn mark_feed_items_read(
    pool: &SqlitePool,
    feed_id: i64,
    newest_item_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) \
         WHERE feed_id = ? AND id <= ?",
    )
    .bind(feed_id)
    .bind(newest_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns an article id resolved from the v1-2 feed id and guid hash pair.
pub async fn article_id_by_guid_hash(
    pool: &SqlitePool,
    feed_id: i64,
    guid_hash: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM article WHERE feed_id = ? AND guid_hash = ? LIMIT 1")
        .bind(feed_id)
        .bind(guid_hash)
        .fetch_optional(pool)
        .await
}

/// Returns how many article ids from the provided list already exist.
pub async fn existing_article_count(
    pool: &SqlitePool,
    item_ids: &[i64],
) -> Result<i64, sqlx::Error> {
    let mut qb: QueryBuilder<'_, Sqlite> =
        QueryBuilder::new("SELECT COUNT(*) FROM article WHERE id IN (");
    {
        let mut separated = qb.separated(", ");
        for id in item_ids {
            separated.push_bind(*id);
        }
    }
    qb.push(")");

    qb.build_query_scalar().fetch_one(pool).await
}

/// Updates one boolean article flag and bumps `last_modified` for the provided ids.
pub async fn update_article_flags(
    pool: &SqlitePool,
    item_ids: &[i64],
    flag: ArticleFlag,
    value: bool,
) -> Result<(), sqlx::Error> {
    let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("UPDATE article SET ");
    qb.push(flag.column_name());
    qb.push(" = ");
    qb.push_bind(value);
    qb.push(", last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE id IN (");
    {
        let mut separated = qb.separated(", ");
        for id in item_ids {
            separated.push_bind(*id);
        }
    }
    qb.push(")");

    qb.build().execute(pool).await?;
    Ok(())
}

/// Marks all items as read up to the provided newest item boundary.
pub async fn mark_all_items_read(
    pool: &SqlitePool,
    newest_item_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE article SET unread = 0, last_modified = CAST(strftime('%s','now') AS INTEGER) WHERE id <= ?",
    )
    .bind(newest_item_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Inserts a mailing-list feed and returns the new identifier.
pub async fn create_mailing_list_feed(
    pool: &SqlitePool,
    from_address: &str,
    feed_title: &str,
    root_id: i64,
    now_ts: i64,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO feed (url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (?, ?, NULL, ?, NULL, ?, 0, NULL, 0, 0, NULL, 1, NULL, 0, 0)",
    )
    .bind(from_address)
    .bind(feed_title)
    .bind(now_ts)
    .bind(root_id)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Persists a fully prepared article record.
pub async fn insert_article_record(
    pool: &SqlitePool,
    article: ArticleRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO article (title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (?, ?, ?, ?, NULL, NULL, ?, NULL, ?, ?, ?, NULL, ?, ?, 0, ?, ?, ?, ?, ?)",
    )
    .bind(article.title)
    .bind(article.content)
    .bind(article.author)
    .bind(article.content_hash)
    .bind(article.feed_id)
    .bind(article.guid)
    .bind(&article.guid_hash)
    .bind(article.last_modified)
    .bind(article.media_thumbnail)
    .bind(article.pub_date)
    .bind(article.starred)
    .bind(article.unread)
    .bind(article.updated_date)
    .bind(article.url)
    .bind(article.summary)
    .execute(pool)
    .await?;
    Ok(())
}
