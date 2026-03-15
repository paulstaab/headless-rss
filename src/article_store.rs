//! Shared article draft building and persistence helpers.

use std::time::{SystemTime, UNIX_EPOCH};

use feed_rs::model::Entry;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::content::{self, FeedContentState};

/// Internal representation used before persisting an article row.
#[derive(Debug, Clone)]
pub struct ArticleRecord {
    pub title: Option<String>,
    pub content: Option<String>,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub feed_id: i64,
    pub guid: String,
    pub guid_hash: String,
    pub last_modified: i64,
    pub media_thumbnail: Option<String>,
    pub pub_date: Option<i64>,
    pub updated_date: Option<i64>,
    pub url: Option<String>,
    pub starred: bool,
    pub unread: bool,
}

/// Outcome of attempting to persist an article by guid hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertArticleOutcome {
    Inserted { guid_hash: String },
    Duplicate { guid_hash: String },
}

/// Computes the MD5 guid hash used as the stable article de-duplication key.
pub fn guid_hash(guid: &str) -> String {
    format!("{:x}", md5::compute(guid.as_bytes()))
}

/// Returns the guid used for a feed entry, falling back to link or title when needed.
pub fn guid_from_feed_entry(entry: &Entry) -> Option<String> {
    if entry.id.is_empty() {
        return entry
            .links
            .first()
            .map(|link| link.href.clone())
            .or_else(|| entry.title.as_ref().map(|title| title.content.clone()));
    }

    Some(entry.id.clone())
}

/// Builds an article record from a feed entry using the current content-quality flags.
pub async fn article_record_from_feed_entry(
    article_http_client: &reqwest::Client,
    config: &Config,
    feed_id: i64,
    entry: &Entry,
    content_state: FeedContentState,
) -> Option<ArticleRecord> {
    let guid = guid_from_feed_entry(entry)?;
    let content = entry
        .content
        .as_ref()
        .and_then(|content| content.body.clone());
    let summary = entry
        .summary
        .as_ref()
        .map(|summary| summary.content.clone());
    let title = entry.title.as_ref().map(|title| title.content.clone());
    let url = entry.links.first().map(|link| link.href.clone());
    let author = entry.authors.first().map(|author| author.name.clone());
    let now_ts = unix_now();
    let updated = entry.updated.map(|dt| dt.timestamp()).unwrap_or(now_ts);
    let published = entry.published.map(|dt| dt.timestamp()).unwrap_or(updated);
    let enriched = content::enrich_article_content(
        article_http_client,
        config,
        Some(feed_id),
        None,
        url.as_deref(),
        content,
        summary,
        None,
        content_state.use_extracted_fulltext,
        content_state.use_llm_summary,
    )
    .await;

    Some(ArticleRecord {
        title,
        content: enriched.content,
        author,
        summary: enriched.summary,
        content_hash: enriched.content_hash,
        feed_id,
        guid_hash: guid_hash(&guid),
        guid,
        last_modified: now_ts,
        media_thumbnail: enriched.media_thumbnail,
        pub_date: Some(published),
        updated_date: Some(updated),
        url,
        starred: false,
        unread: true,
    })
}

/// Inserts an article when its guid hash does not already exist.
pub async fn insert_article_if_new(
    pool: &SqlitePool,
    article: ArticleRecord,
) -> Result<InsertArticleOutcome, sqlx::Error> {
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM article WHERE guid_hash = ? LIMIT 1")
            .bind(&article.guid_hash)
            .fetch_optional(pool)
            .await?;

    if existing.is_some() {
        return Ok(InsertArticleOutcome::Duplicate {
            guid_hash: article.guid_hash,
        });
    }

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

    Ok(InsertArticleOutcome::Inserted {
        guid_hash: article.guid_hash,
    })
}

/// Returns the current unix timestamp in seconds.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
