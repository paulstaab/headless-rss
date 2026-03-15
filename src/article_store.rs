//! Shared article draft building and persistence helpers.

use std::time::{SystemTime, UNIX_EPOCH};

use feed_rs::model::Entry;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::content::{self, ArticleContentContext, ArticleContentPayload, FeedContentState};
use crate::repo;

/// Internal representation of a new article before persistence.
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

/// Shared context for enriching and persisting a candidate article.
///
/// The feed URL is optional so newsletter ingestion can reuse the same pipeline without
/// pretending that IMAP-derived items came from a fetchable feed origin.
pub struct ArticleIngestionContext<'a> {
    pub pool: &'a SqlitePool,
    pub article_http_client: &'a reqwest::Client,
    pub config: &'a Config,
    pub feed_url: Option<&'a str>,
    pub content_state: FeedContentState,
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

/// Builds a new article record from a feed entry before any enrichment work runs.
pub fn article_record_from_feed_entry(feed_id: i64, entry: &Entry) -> Option<ArticleRecord> {
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

    Some(ArticleRecord {
        title,
        content,
        author,
        summary,
        content_hash: None,
        feed_id,
        guid_hash: guid_hash(&guid),
        guid,
        last_modified: now_ts,
        media_thumbnail: None,
        pub_date: Some(published),
        updated_date: Some(updated),
        url,
        starred: false,
        unread: true,
    })
}

/// Returns whether an article with the given guid hash is already persisted.
pub async fn article_exists_by_guid_hash(
    pool: &SqlitePool,
    guid_hash: &str,
) -> Result<bool, sqlx::Error> {
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM article WHERE guid_hash = ? LIMIT 1")
            .bind(guid_hash)
            .fetch_optional(pool)
            .await?;

    Ok(existing.is_some())
}

/// Applies extraction and summary generation to a new article after duplicate checks pass.
pub async fn enrich_article_record(
    article_http_client: &reqwest::Client,
    config: &Config,
    feed_url: Option<&str>,
    content_state: FeedContentState,
    article: ArticleRecord,
) -> ArticleRecord {
    let enriched = content::enrich_article_content(
        ArticleContentContext {
            article_http_client,
            config,
            feed_id: Some(article.feed_id),
            article_id: None,
            feed_url,
            article_url: article.url.as_deref(),
        },
        ArticleContentPayload {
            content: article.content,
            summary: article.summary,
            media_thumbnail: article.media_thumbnail,
            use_extracted_fulltext: content_state.use_extracted_fulltext,
            use_llm_summary: content_state.use_llm_summary,
        },
    )
    .await;

    ArticleRecord {
        content: enriched.content,
        summary: enriched.summary,
        content_hash: enriched.content_hash,
        media_thumbnail: enriched.media_thumbnail,
        ..article
    }
}

/// Enriches a new article and persists it when its guid hash is not already stored.
pub async fn ingest_article_if_new(
    context: &ArticleIngestionContext<'_>,
    article: ArticleRecord,
) -> Result<InsertArticleOutcome, sqlx::Error> {
    if article_exists_by_guid_hash(context.pool, &article.guid_hash).await? {
        return Ok(InsertArticleOutcome::Duplicate {
            guid_hash: article.guid_hash,
        });
    }

    let article = enrich_article_record(
        context.article_http_client,
        context.config,
        context.feed_url,
        context.content_state,
        article,
    )
    .await;

    repo::insert_article_record(context.pool, article.clone()).await?;

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
