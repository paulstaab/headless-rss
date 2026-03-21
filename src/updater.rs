//! Scheduled feed refresh logic, dynamic cadence calculation, and stale-article cleanup.

use anyhow::{Context, Result};
use feed_rs::model::Feed;
use rand::RngExt;
use sqlx::{FromRow, SqlitePool};

use crate::article_store::{self, InsertArticleOutcome};
use crate::config::Config;
use crate::content::{self, FeedContentState};
use crate::db;
use crate::email;
use crate::http_client;
use crate::ssrf;

/// Jitter window (in seconds) used for low-activity feeds.
///
/// This spreads daily checks by +/-30 minutes so many feeds do not refresh at the same timestamp.
const THIRTY_MINUTES: i64 = 1_800;
/// Maximum interval for active feeds.
///
/// For active feeds we may compute shorter intervals, but never wait longer than 12 hours.
const TWELVE_HOURS: i64 = 43_200;
/// One day in seconds.
const ONE_DAY: i64 = 86_400;
/// Retention window for stale feed-article cleanup.
const NINETY_DAYS: i64 = 90 * ONE_DAY;

/// Projection of the feed metadata needed to decide whether and how a feed should be refreshed.
#[derive(FromRow)]
struct FeedToUpdate {
    id: i64,
    url: String,
    title: Option<String>,
    last_quality_check: Option<i64>,
    use_extracted_fulltext: bool,
    use_llm_summary: bool,
    manual_use_extracted_fulltext: Option<bool>,
    manual_use_llm_summary: Option<bool>,
}

/// Shared context for ingesting entries from one feed refresh cycle.
struct FeedEntryIngestionContext<'a> {
    pool: &'a SqlitePool,
    article_http_client: &'a reqwest::Client,
    config: &'a Config,
    feed_id: i64,
    feed_url: &'a str,
    content_state: FeedContentState,
}

/// Human-readable result of a forced feed-quality re-evaluation.
#[derive(Debug)]
pub struct FeedQualityReevaluationResult {
    pub feed_id: i64,
    pub feed_title: Option<String>,
    pub use_extracted_fulltext: bool,
    pub use_llm_summary: bool,
    pub manual_use_extracted_fulltext: Option<bool>,
    pub manual_use_llm_summary: Option<bool>,
    pub last_quality_check: Option<i64>,
    pub last_manual_quality_override: Option<i64>,
}

/// Runs one foreground update cycle for the CLI `update` command.
pub async fn update_all(config: &Config) -> Result<()> {
    tracing::info!("starting rust feed update cycle");
    let pool = db::create_pool(&config.db_path)
        .await
        .with_context(|| format!("failed to connect to sqlite db at {}", config.db_path))?;
    let updated = update_due_feeds(&pool, config, config.testing_mode).await?;
    tracing::info!(updated, "finished rust feed update cycle");
    Ok(())
}

/// Re-evaluates one regular feed's full-text and summary quality flags immediately.
///
/// This command fetches and parses the selected feed, forces the quality-check path to run even
/// when the last monthly evaluation is still fresh, and persists only the feed-level quality
/// flags. It does not ingest articles or recalculate the feed cadence.
pub async fn reevaluate_feed_quality(
    config: &Config,
    feed_id: i64,
) -> Result<FeedQualityReevaluationResult> {
    let pool = db::create_pool(&config.db_path)
        .await
        .with_context(|| format!("failed to connect to sqlite db at {}", config.db_path))?;
    reevaluate_single_feed_quality(&pool, config, feed_id, config.testing_mode).await
}

/// Manually sets one or both feed-level quality flags and locks only the provided attributes.
///
/// The selected attribute overrides are persisted as manual values so future automatic monthly
/// quality checks keep those attributes fixed while continuing to evaluate any unlocked
/// attributes. The command also updates the effective feed flags and stamps both the quality-check
/// time and the manual-override time.
pub async fn set_feed_quality_overrides(
    config: &Config,
    feed_id: i64,
    use_extracted_fulltext: Option<bool>,
    use_llm_summary: Option<bool>,
) -> Result<FeedQualityReevaluationResult> {
    anyhow::ensure!(
        use_extracted_fulltext.is_some() || use_llm_summary.is_some(),
        "at least one of --use-extracted-fulltext or --use-llm-summary must be provided"
    );

    let pool = db::create_pool(&config.db_path)
        .await
        .with_context(|| format!("failed to connect to sqlite db at {}", config.db_path))?;
    set_single_feed_quality_overrides(&pool, feed_id, use_extracted_fulltext, use_llm_summary).await
}

/// Refreshes only feeds whose `next_update_time` is due.
pub async fn update_due_feeds(
    pool: &SqlitePool,
    config: &Config,
    testing_mode: bool,
) -> Result<usize> {
    let now_ts = article_store::unix_now();
    let feeds: Vec<FeedToUpdate> = sqlx::query_as(
        "SELECT id, url, title, last_quality_check, use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary FROM feed WHERE is_mailing_list = 0 AND (next_update_time IS NULL OR next_update_time <= ?)",
    )
    .bind(now_ts)
    .fetch_all(pool)
    .await
    .context("failed to query due feeds")?;

    update_feed_batch(pool, config, testing_mode, feeds, "due").await
}

/// Forces refresh of all non-mailing-list feeds, ignoring the stored schedule.
pub async fn update_all_regular_feeds(
    pool: &SqlitePool,
    config: &Config,
    testing_mode: bool,
) -> Result<usize> {
    let feeds: Vec<FeedToUpdate> =
        sqlx::query_as(
            "SELECT id, url, title, last_quality_check, use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary FROM feed WHERE is_mailing_list = 0",
        )
            .fetch_all(pool)
            .await
            .context("failed to query all regular feeds")?;

    update_feed_batch(pool, config, testing_mode, feeds, "all").await
}

/// Updates a selected batch of feeds and then runs newsletter ingestion/cleanup.
async fn update_feed_batch(
    pool: &SqlitePool,
    config: &Config,
    testing_mode: bool,
    feeds: Vec<FeedToUpdate>,
    batch_kind: &str,
) -> Result<usize> {
    let feed_http_client = http_client::build_feed_http_client()?;
    let article_http_client = http_client::build_article_http_client()?;

    tracing::debug!(
        due_feeds = feeds.len(),
        testing_mode,
        batch_kind,
        "loaded due feeds for update"
    );

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for feed in &feeds {
        if let Err(err) = update_single_feed(
            pool,
            &feed_http_client,
            &article_http_client,
            config,
            feed,
            testing_mode,
        )
        .await
        {
            let detail = err.to_string();
            tracing::warn!(feed_id = feed.id, "feed update failed");
            sqlx::query(
                "UPDATE feed SET update_error_count = update_error_count + 1, last_update_error = ? WHERE id = ?",
            )
            .bind(detail)
            .bind(feed.id)
            .execute(pool)
            .await
            .context("failed to persist feed update error")?;
            failed += 1;
        } else {
            succeeded += 1;
        }
    }

    tracing::info!(
        due_feeds = feeds.len(),
        succeeded,
        failed,
        batch_kind,
        "feed update batch summary"
    );

    email::fetch_emails_from_all_mailboxes(pool, config).await?;

    Ok(feeds.len())
}

/// Refreshes one feed, ingests up to 50 entries, updates dynamic cadence, and clears error state.
async fn update_single_feed(
    pool: &SqlitePool,
    feed_http_client: &reqwest::Client,
    article_http_client: &reqwest::Client,
    config: &Config,
    feed: &FeedToUpdate,
    testing_mode: bool,
) -> Result<()> {
    let feed_id = feed.id;
    let url = feed.url.as_str();
    tracing::debug!(feed_id, testing_mode, "starting feed update");
    let parsed = fetch_and_parse_feed(feed_http_client, url, testing_mode).await?;
    let content_state = content::maybe_refresh_feed_content_state(
        pool,
        article_http_client,
        config,
        feed_id,
        &feed.url,
        FeedContentState {
            last_quality_check: feed.last_quality_check,
            use_extracted_fulltext: feed.use_extracted_fulltext,
            use_llm_summary: feed.use_llm_summary,
            manual_use_extracted_fulltext: feed.manual_use_extracted_fulltext,
            manual_use_llm_summary: feed.manual_use_llm_summary,
        },
        &parsed.entries,
    )
    .await?;

    let mut inserted = 0usize;
    let mut processed = 0usize;
    let mut current_feed_guid_hashes = Vec::new();
    let entry_context = FeedEntryIngestionContext {
        pool,
        article_http_client,
        config,
        feed_id,
        feed_url: &feed.url,
        content_state,
    };

    for entry in parsed.entries.iter().take(50) {
        processed += 1;
        if insert_article_from_entry(&entry_context, entry, &mut current_feed_guid_hashes).await? {
            inserted += 1;
        }
    }

    let removed = cleanup_stale_feed_articles(pool, feed_id, &current_feed_guid_hashes).await?;

    let now_ts = article_store::unix_now();
    let next_update_time = calculate_next_update_time(pool, feed_id, now_ts).await?;
    sqlx::query(
        "UPDATE feed SET update_error_count = 0, last_update_error = NULL, next_update_time = ? WHERE id = ?",
    )
    .bind(next_update_time)
    .bind(feed_id)
    .execute(pool)
    .await
    .context("failed to update feed metadata")?;

    let skipped = processed.saturating_sub(inserted);
    tracing::info!(
        feed_id,
        processed,
        inserted,
        skipped,
        removed,
        "feed update completed"
    );
    Ok(())
}

/// Fetches one regular feed and forces a fresh content-quality evaluation.
async fn reevaluate_single_feed_quality(
    pool: &SqlitePool,
    config: &Config,
    feed_id: i64,
    testing_mode: bool,
) -> Result<FeedQualityReevaluationResult> {
    let feed_http_client = http_client::build_feed_http_client()?;
    let article_http_client = http_client::build_article_http_client()?;
    let feed: FeedToUpdate = sqlx::query_as(
        "SELECT id, url, title, last_quality_check, use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary FROM feed WHERE id = ? AND is_mailing_list = 0",
    )
    .bind(feed_id)
    .fetch_optional(pool)
    .await
    .context("failed to load feed for quality re-evaluation")?
    .with_context(|| format!("feed {feed_id} not found"))?;
    let parsed = fetch_and_parse_feed(&feed_http_client, &feed.url, testing_mode).await?;
    tracing::info!(
        feed_id,
        "starting forced feed content quality re-evaluation"
    );

    sqlx::query(
        "UPDATE feed SET manual_use_extracted_fulltext = NULL, manual_use_llm_summary = NULL, last_manual_quality_override = NULL WHERE id = ?",
    )
    .bind(feed_id)
    .execute(pool)
    .await
    .context("failed to clear manual feed quality overrides")?;

    let next_state = content::maybe_refresh_feed_content_state(
        pool,
        &article_http_client,
        config,
        feed.id,
        &feed.url,
        FeedContentState {
            last_quality_check: None,
            use_extracted_fulltext: feed.use_extracted_fulltext,
            use_llm_summary: feed.use_llm_summary,
            manual_use_extracted_fulltext: None,
            manual_use_llm_summary: None,
        },
        &parsed.entries,
    )
    .await?;

    tracing::info!(
        feed_id,
        "finished forced feed content quality re-evaluation"
    );
    Ok(FeedQualityReevaluationResult {
        feed_id,
        feed_title: feed.title,
        use_extracted_fulltext: next_state.use_extracted_fulltext,
        use_llm_summary: next_state.use_llm_summary,
        manual_use_extracted_fulltext: next_state.manual_use_extracted_fulltext,
        manual_use_llm_summary: next_state.manual_use_llm_summary,
        last_quality_check: next_state.last_quality_check,
        last_manual_quality_override: None,
    })
}

/// Persists manual overrides for one regular feed without fetching the remote document.
async fn set_single_feed_quality_overrides(
    pool: &SqlitePool,
    feed_id: i64,
    use_extracted_fulltext: Option<bool>,
    use_llm_summary: Option<bool>,
) -> Result<FeedQualityReevaluationResult> {
    let feed: FeedToUpdate = sqlx::query_as(
        "SELECT id, url, title, last_quality_check, use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary FROM feed WHERE id = ? AND is_mailing_list = 0",
    )
    .bind(feed_id)
    .fetch_optional(pool)
    .await
    .context("failed to load feed for manual quality override")?
    .with_context(|| format!("feed {feed_id} not found"))?;

    let now = article_store::unix_now();
    let manual_use_extracted_fulltext =
        use_extracted_fulltext.or(feed.manual_use_extracted_fulltext);
    let manual_use_llm_summary = use_llm_summary.or(feed.manual_use_llm_summary);
    let effective_use_extracted_fulltext =
        use_extracted_fulltext.unwrap_or(feed.use_extracted_fulltext);
    let effective_use_llm_summary = use_llm_summary.unwrap_or(feed.use_llm_summary);

    sqlx::query(
        "UPDATE feed SET use_extracted_fulltext = ?, use_llm_summary = ?, manual_use_extracted_fulltext = ?, manual_use_llm_summary = ?, last_quality_check = ?, last_manual_quality_override = ? WHERE id = ?",
    )
    .bind(effective_use_extracted_fulltext)
    .bind(effective_use_llm_summary)
    .bind(manual_use_extracted_fulltext)
    .bind(manual_use_llm_summary)
    .bind(now)
    .bind(now)
    .bind(feed_id)
    .execute(pool)
    .await
    .context("failed to persist manual feed quality overrides")?;

    Ok(FeedQualityReevaluationResult {
        feed_id,
        feed_title: feed.title,
        use_extracted_fulltext: effective_use_extracted_fulltext,
        use_llm_summary: effective_use_llm_summary,
        manual_use_extracted_fulltext,
        manual_use_llm_summary,
        last_quality_check: Some(now),
        last_manual_quality_override: Some(now),
    })
}

/// Downloads a remote feed document and parses it into the shared feed model.
async fn fetch_and_parse_feed(
    feed_http_client: &reqwest::Client,
    url: &str,
    testing_mode: bool,
) -> Result<Feed> {
    let response = ssrf::get_with_safe_redirects(feed_http_client, url, testing_mode)
        .await
        .map_err(ssrf::SafeGetError::into_anyhow)
        .with_context(|| format!("request failed for {url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("request failed for {url}: HTTP {}", response.status());
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read response body")?;
    feed_rs::parser::parse(&bytes[..]).context("failed to parse feed")
}

/// Removes stale feed articles that are no longer present in the latest payload.
///
/// Articles are only eligible when they are old, read, and unstarred so refreshes do not
/// delete recent unread content or user-saved items.
async fn cleanup_stale_feed_articles(
    pool: &SqlitePool,
    feed_id: i64,
    current_feed_guid_hashes: &[String],
) -> Result<u64> {
    let stale_before = article_store::unix_now() - NINETY_DAYS;

    let result = if current_feed_guid_hashes.is_empty() {
        sqlx::query(
            "DELETE FROM article WHERE feed_id = ? AND last_modified < ? AND unread = 0 AND starred = 0",
        )
        .bind(feed_id)
        .bind(stale_before)
        .execute(pool)
        .await
        .context("failed stale article cleanup query")?
    } else {
        let placeholders = vec!["?"; current_feed_guid_hashes.len()].join(", ");
        let query = format!(
            "DELETE FROM article WHERE feed_id = ? AND last_modified < ? AND unread = 0 AND starred = 0 AND guid_hash NOT IN ({placeholders})"
        );

        let mut cleanup_query = sqlx::query(&query).bind(feed_id).bind(stale_before);
        for guid_hash in current_feed_guid_hashes {
            cleanup_query = cleanup_query.bind(guid_hash);
        }

        cleanup_query
            .execute(pool)
            .await
            .context("failed stale article cleanup query")?
    };

    Ok(result.rows_affected())
}

/// Calculates the next refresh timestamp from the last 7 days of observed article output.
async fn calculate_next_update_time(pool: &SqlitePool, feed_id: i64, now_ts: i64) -> Result<i64> {
    // Derive cadence from recent output over the last 7 days.
    let weekly_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = ? AND pub_date > ?")
            .bind(feed_id)
            .bind(now_ts - 7 * ONE_DAY)
            .fetch_one(pool)
            .await
            .context("failed to query recent article frequency")?;

    let avg_articles_per_day = weekly_count as f64 / 7.0;
    let next_update_in =
        compute_next_update_interval(avg_articles_per_day, random_jitter_seconds());

    tracing::info!(
        feed_id,
        avg_articles_per_day,
        next_update_in_minutes = (next_update_in as f64 / 60.0),
        "calculated next dynamic update time"
    );

    Ok(now_ts + next_update_in)
}

/// Returns a signed jitter value in seconds in [-30m, +30m].
///
/// This is only applied to sparse feeds to avoid synchronized daily polling.
fn random_jitter_seconds() -> i64 {
    let mut rng = rand::rng();
    rng.random_range(-THIRTY_MINUTES..=THIRTY_MINUTES)
}

/// Computes the next refresh interval in seconds from recent publishing frequency.
///
/// Cleanup policy for stale feed articles:
/// - Sparse feeds ($\le 0.1$ articles/day): refresh roughly daily with +/-30m jitter.
/// - Active feeds: refresh at 4x observed daily rate, capped so interval is at most 12h.
fn compute_next_update_interval(avg_articles_per_day: f64, jitter_seconds: i64) -> i64 {
    if avg_articles_per_day <= 0.1 {
        return ONE_DAY + jitter_seconds.clamp(-THIRTY_MINUTES, THIRTY_MINUTES);
    }

    ((ONE_DAY as f64 / avg_articles_per_day / 4.0).round() as i64).min(TWELVE_HOURS)
}

/// Converts one parsed feed entry into a persisted article when it is not a duplicate.
async fn insert_article_from_entry(
    entry_context: &FeedEntryIngestionContext<'_>,
    entry: &feed_rs::model::Entry,
    current_feed_guid_hashes: &mut Vec<String>,
) -> Result<bool> {
    let Some(article) = article_store::article_record_from_feed_entry(entry_context.feed_id, entry)
    else {
        tracing::debug!(
            feed_id = entry_context.feed_id,
            "skipping entry without guid/link/title"
        );
        return Ok(false);
    };

    current_feed_guid_hashes.push(article.guid_hash.clone());

    let ingestion_context = article_store::ArticleIngestionContext {
        pool: entry_context.pool,
        article_http_client: entry_context.article_http_client,
        config: entry_context.config,
        feed_url: Some(entry_context.feed_url),
        content_state: entry_context.content_state,
    };

    match article_store::ingest_article_if_new(&ingestion_context, article)
        .await
        .context("failed to insert article")?
    {
        InsertArticleOutcome::Inserted { .. } => Ok(true),
        InsertArticleOutcome::Duplicate { guid_hash } => {
            tracing::debug!(
                feed_id = entry_context.feed_id,
                guid_hash,
                "skipping duplicate entry"
            );
            Ok(false)
        }
    }
}

#[cfg(test)]
fn unix_now() -> i64 {
    article_store::unix_now()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::http::header as http_header;
    use axum::http::{HeaderMap as AxumHeaderMap, StatusCode};
    use axum::routing::{get, post};
    use serde_json::json;
    use sqlx::SqlitePool;
    use tokio::net::TcpListener;

    use crate::config::Config;

    use super::{
        NINETY_DAYS, ONE_DAY, THIRTY_MINUTES, TWELVE_HOURS, compute_next_update_interval, unix_now,
    };
    use super::{
        reevaluate_single_feed_quality, set_single_feed_quality_overrides,
        update_all_regular_feeds, update_due_feeds,
    };

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE feed (id INTEGER PRIMARY KEY NOT NULL, url VARCHAR NOT NULL UNIQUE, title VARCHAR, favicon_link VARCHAR, added INTEGER NOT NULL, next_update_time INTEGER, folder_id INTEGER NOT NULL, ordering INTEGER NOT NULL, link VARCHAR, pinned BOOLEAN NOT NULL, update_error_count INTEGER NOT NULL, last_update_error VARCHAR, is_mailing_list BOOLEAN NOT NULL DEFAULT 0, last_quality_check INTEGER, use_extracted_fulltext BOOLEAN NOT NULL DEFAULT 0, use_llm_summary BOOLEAN NOT NULL DEFAULT 0, manual_use_extracted_fulltext BOOLEAN, manual_use_llm_summary BOOLEAN, last_manual_quality_override INTEGER)",
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
        sqlx::query(
            "CREATE TABLE email_credentials (id INTEGER PRIMARY KEY AUTOINCREMENT, protocol VARCHAR NOT NULL, server VARCHAR NOT NULL, port INTEGER NOT NULL, username VARCHAR NOT NULL, password VARCHAR NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn test_config() -> Config {
        Config {
            username: None,
            password: None,
            version: "dev".to_string(),
            db_path: ":memory:".to_string(),
            feed_update_frequency_min: 15,
            openai_api_key: None,
            openai_base_url: "https://api.openai.com/v1".to_string(),
            openai_model: "gpt-5-nano".to_string(),
            openai_timeout_seconds: 30,
            testing_mode: true,
        }
    }

    fn test_config_with_llm(base_url: &str) -> Config {
        Config {
            openai_api_key: Some("test-key".to_string()),
            openai_base_url: format!("{base_url}/v1"),
            openai_model: "test-model".to_string(),
            ..test_config()
        }
    }

    async fn start_fixture_feed_server() -> String {
        let app = Router::new().route(
            "/atom.xml",
            get(|| async {
                (
                    [(http_header::CONTENT_TYPE, "application/atom+xml")],
                    r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Updater Fixture</title>
  <link href="http://example.org/" />
  <updated>2026-03-06T00:00:00Z</updated>
  <id>tag:example.org,2026:feed</id>
  <entry>
    <title>Update Entry</title>
    <link href="http://example.org/update-entry" />
    <id>tag:example.org,2026:update-entry</id>
    <updated>2026-03-06T00:00:00Z</updated>
    <summary>Update summary</summary>
        <content type="html"><![CDATA[<p>Body</p><img src="https://example.org/thumb.jpg" alt="thumb" />]]></content>
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

    async fn start_fixture_feed_server_requiring_headers() -> String {
        let app = Router::new().route(
            "/atom.xml",
            get(|headers: AxumHeaderMap| async move {
                let user_agent_ok = headers
                    .get(http_header::USER_AGENT)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| {
                        value.contains("headless-rss") || value.contains("Mozilla/5.0")
                    });
                let accept_ok = headers
                    .get(http_header::ACCEPT)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| {
                        value.contains("application/rss+xml")
                            || value.contains("application/atom+xml")
                    });

                if !(user_agent_ok && accept_ok) {
                    return (StatusCode::FORBIDDEN, "blocked");
                }

                (
                    StatusCode::OK,
                    r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Updater Fixture</title>
  <link href="http://example.org/" />
  <updated>2026-03-06T00:00:00Z</updated>
  <id>tag:example.org,2026:feed</id>
  <entry>
    <title>Header Guard Entry</title>
    <link href="http://example.org/header-guard-entry" />
    <id>tag:example.org,2026:header-guard-entry</id>
    <updated>2026-03-06T00:00:00Z</updated>
    <summary>Update summary</summary>
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

    #[tokio::test]
    async fn update_due_feeds_inserts_new_articles() {
        let pool = setup_pool().await;
        let url = start_fixture_feed_server().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (1, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let now_before = unix_now();
        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        let now_after = unix_now();
        assert_eq!(updated, 1);

        let article_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(article_count, 1);

        let err_count: i64 = sqlx::query_scalar("SELECT update_error_count FROM feed WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(err_count, 0);

        let next_update_time: i64 =
            sqlx::query_scalar("SELECT next_update_time FROM feed WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        let min_expected = now_before + ONE_DAY - THIRTY_MINUTES - 2;
        let max_expected = now_after + ONE_DAY + THIRTY_MINUTES + 2;
        assert!(
            (min_expected..=max_expected).contains(&next_update_time),
            "next_update_time={next_update_time}, expected range [{min_expected}, {max_expected}]"
        );
    }

    #[test]
    fn compute_next_update_interval_daily_when_feed_is_sparse() {
        let with_negative_jitter = compute_next_update_interval(0.1, -THIRTY_MINUTES);
        let with_positive_jitter = compute_next_update_interval(0.0, THIRTY_MINUTES);

        assert_eq!(with_negative_jitter, ONE_DAY - THIRTY_MINUTES);
        assert_eq!(with_positive_jitter, ONE_DAY + THIRTY_MINUTES);
    }

    #[test]
    fn compute_next_update_interval_uses_cap_for_recent_activity() {
        let interval = compute_next_update_interval(1.0 / 7.0, 0);
        assert_eq!(interval, TWELVE_HOURS);
    }

    #[test]
    fn compute_next_update_interval_scales_with_high_activity() {
        let interval = compute_next_update_interval(10.0, 0);
        assert_eq!(interval, 2_160);
    }

    #[tokio::test]
    async fn update_due_feeds_extracts_media_thumbnail_from_entry_body() {
        let pool = setup_pool().await;
        let url = start_fixture_feed_server().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (4, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        assert_eq!(updated, 1);

        let thumbnail: Option<String> =
            sqlx::query_scalar("SELECT media_thumbnail FROM article WHERE feed_id = 4 LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(thumbnail.as_deref(), Some("https://example.org/thumb.jpg"));
    }

    #[tokio::test]
    async fn update_due_feeds_sends_feed_headers() {
        let pool = setup_pool().await;
        let url = start_fixture_feed_server_requiring_headers().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (5, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        assert_eq!(updated, 1);

        let article_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = 5")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(article_count, 1);

        let err_count: i64 = sqlx::query_scalar("SELECT update_error_count FROM feed WHERE id = 5")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(err_count, 0);
    }

    #[tokio::test]
    async fn update_due_feeds_persists_errors() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (2, 'file:///etc/passwd', 'Bad', NULL, 1, 0, 1, 0, NULL, 0, 0, NULL, 0, NULL, 0, 0)")
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), false)
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let err_count: i64 = sqlx::query_scalar("SELECT update_error_count FROM feed WHERE id = 2")
            .fetch_one(&pool)
            .await
            .unwrap();
        let err_detail: Option<String> =
            sqlx::query_scalar("SELECT last_update_error FROM feed WHERE id = 2")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(err_count, 1);
        assert!(
            err_detail
                .unwrap_or_default()
                .contains("request failed for file:///etc/passwd")
        );
    }

    #[tokio::test]
    async fn update_due_feeds_skips_mailing_list_rows() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (3, 'newsletter@example.com', 'News', NULL, 1, 0, 1, 0, NULL, 0, 0, NULL, 1, NULL, 0, 0)")
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), false)
            .await
            .unwrap();
        assert_eq!(updated, 0);

        let err_count: i64 = sqlx::query_scalar("SELECT update_error_count FROM feed WHERE id = 3")
            .fetch_one(&pool)
            .await
            .unwrap();
        let err_detail: Option<String> =
            sqlx::query_scalar("SELECT last_update_error FROM feed WHERE id = 3")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(err_count, 0);
        assert_eq!(err_detail, None);
    }

    #[tokio::test]
    async fn update_all_regular_feeds_ignores_next_update_time_gate() {
        let pool = setup_pool().await;
        let url = start_fixture_feed_server().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (6, ?, 'Updater Fixture', NULL, 1, 9999999999, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_all_regular_feeds(&pool, &test_config(), true)
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let article_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = 6")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(article_count, 1);
    }

    #[tokio::test]
    async fn update_due_feeds_cleans_only_eligible_stale_articles() {
        let pool = setup_pool().await;
        let url = start_fixture_feed_server().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (7, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let now = unix_now();
        let stale = now - (NINETY_DAYS + ONE_DAY);
        let fresh = now - ONE_DAY;
        let in_payload_guid = "tag:example.org,2026:update-entry";
        let in_payload_hash = format!("{:x}", md5::compute(in_payload_guid.as_bytes()));

        insert_test_article(&pool, 7, "stale-delete", stale, 0, 0).await;
        insert_test_article(&pool, 7, "stale-unread", stale, 1, 0).await;
        insert_test_article(&pool, 7, "stale-starred", stale, 0, 1).await;
        insert_test_article(&pool, 7, "fresh-read-unstarred", fresh, 0, 0).await;
        insert_test_article_with_hash(&pool, 7, in_payload_guid, &in_payload_hash, stale, 0, 0)
            .await;

        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        assert_eq!(updated, 1);

        let deleted_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM article WHERE guid = 'stale-delete' LIMIT 1")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(deleted_exists.is_none());

        let unread_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM article WHERE guid = 'stale-unread' LIMIT 1")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(unread_exists.is_some());

        let starred_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM article WHERE guid = 'stale-starred' LIMIT 1")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(starred_exists.is_some());

        let fresh_exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM article WHERE guid = 'fresh-read-unstarred' LIMIT 1",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(fresh_exists.is_some());

        let in_payload_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM article WHERE guid_hash = ? LIMIT 1")
                .bind(in_payload_hash)
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(in_payload_exists.is_some());
    }

    #[tokio::test]
    async fn update_due_feeds_enables_extracted_fulltext_when_quality_is_better() {
        let pool = setup_pool().await;
        let url = start_quality_fixture_feed_server(
            Some("Short teaser."),
            None,
            "<html><body><article><p>This is a long extracted article body with enough detail to clearly exceed the teaser summary in the feed.</p><p>It contains additional explanation and supporting context.</p></article><footer>Footer</footer></body></html>",
            None,
        )
        .await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (8, ?, 'Quality Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        assert_eq!(updated, 1);

        let flags: (bool, bool, Option<i64>) = sqlx::query_as(
            "SELECT use_extracted_fulltext, use_llm_summary, last_quality_check FROM feed WHERE id = 8",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(flags.0);
        assert!(!flags.1);
        assert!(flags.2.is_some());

        let content: Option<String> =
            sqlx::query_scalar("SELECT content FROM article WHERE feed_id = 8 LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            content
                .unwrap_or_default()
                .contains("long extracted article body")
        );
    }

    #[tokio::test]
    async fn update_due_feeds_leaves_extraction_disabled_when_quality_is_not_better() {
        let pool = setup_pool().await;
        let url = start_quality_fixture_feed_server(
            Some(
                "This feed summary already includes substantial detail about the article contents.",
            ),
            None,
            "<html><body><article><p>Short article.</p></article></body></html>",
            None,
        )
        .await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (9, ?, 'Quality Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        assert_eq!(updated, 1);

        let flags: (bool, bool, Option<i64>) = sqlx::query_as(
            "SELECT use_extracted_fulltext, use_llm_summary, last_quality_check FROM feed WHERE id = 9",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!flags.0);
        assert!(!flags.1);
        assert!(flags.2.is_some());
    }

    #[tokio::test]
    async fn update_due_feeds_enables_llm_summary_by_heuristic_when_summary_is_prefix() {
        let pool = setup_pool().await;
        let url = start_quality_fixture_feed_server(
            Some("Lead sentence only."),
            Some("<p>Lead sentence only. More detail follows in the full article body.</p>"),
            "<html><body><article><p>Short article.</p></article></body></html>",
            None,
        )
        .await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (10, ?, 'Quality Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config(), true).await.unwrap();
        assert_eq!(updated, 1);

        let flags: (bool, bool) = sqlx::query_as(
            "SELECT use_extracted_fulltext, use_llm_summary FROM feed WHERE id = 10",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!flags.0);
        assert!(flags.1);
    }

    #[tokio::test]
    async fn update_due_feeds_disables_llm_summary_when_quality_check_says_summary_is_good() {
        let pool = setup_pool().await;
        let base_url = start_quality_fixture_feed_server(
            Some("A good summary."),
            Some("<p>A good summary. This article contains extra detail for validation.</p>"),
            "<html><body><article><p>Short article.</p></article></body></html>",
            Some(r#"{"is_good":true}"#),
        )
        .await;
        let llm_base_url = base_url.trim_end_matches("/atom.xml").to_string();
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (11, ?, 'Quality Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(base_url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config_with_llm(&llm_base_url), true)
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let flags: (bool, bool) = sqlx::query_as(
            "SELECT use_extracted_fulltext, use_llm_summary FROM feed WHERE id = 11",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!flags.0);
        assert!(!flags.1);
    }

    #[tokio::test]
    async fn update_due_feeds_replaces_feed_summary_when_llm_quality_check_rejects_it() {
        let pool = setup_pool().await;
        let base_url = start_quality_fixture_feed_server(
            Some("Short teaser only."),
            Some("<p>Short teaser only. This full article contains enough additional detail to justify generating a better summary from the language model, rather than keeping the feed-provided teaser text.</p><p>More context follows with concrete implementation notes, edge cases, and additional explanation for validation.</p>"),
            "<html><body><article><p>Short article.</p></article></body></html>",
            Some(r#"{"is_good":false}"#),
        )
        .await;
        let llm_base_url = base_url.trim_end_matches("/atom.xml").to_string();
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (12, ?, 'Quality Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, NULL, 0, 0)")
            .bind(base_url.clone())
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, &test_config_with_llm(&llm_base_url), true)
            .await
            .unwrap();
        assert_eq!(updated, 1);

        let flags: (bool, bool) = sqlx::query_as(
            "SELECT use_extracted_fulltext, use_llm_summary FROM feed WHERE id = 12",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!flags.0);
        assert!(flags.1);

        let summary: Option<String> =
            sqlx::query_scalar("SELECT summary FROM article WHERE feed_id = 12 LIMIT 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            summary.as_deref(),
            Some("Fixture summary from mock LLM. (AI generated)")
        );
    }

    #[tokio::test]
    async fn reevaluate_single_feed_quality_updates_flags_without_ingesting_articles() {
        let pool = setup_pool().await;
        let url = start_quality_fixture_feed_server(
            Some("Short teaser."),
            None,
            "<html><body><article><p>This is a long extracted article body with enough detail to clearly exceed the teaser summary in the feed.</p><p>It contains additional explanation and supporting context.</p></article></body></html>",
            None,
        )
        .await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (14, ?, 'Quality Fixture', NULL, 1, 12345, 1, 0, 'http://example.org', 0, 0, NULL, 0, ?, 0, 0)")
            .bind(url)
            .bind(unix_now())
            .execute(&pool)
            .await
            .unwrap();

        reevaluate_single_feed_quality(&pool, &test_config(), 14, true)
            .await
            .unwrap();

        let flags: (bool, bool, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT use_extracted_fulltext, use_llm_summary, last_quality_check, next_update_time FROM feed WHERE id = 14",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(flags.0);
        assert!(!flags.1);
        assert!(flags.2.is_some());
        assert_eq!(flags.3, Some(12_345));

        let article_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM article WHERE feed_id = 14")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(article_count, 0);
    }

    #[tokio::test]
    async fn set_single_feed_quality_overrides_locks_only_selected_attribute() {
        let pool = setup_pool().await;
        let url = start_quality_fixture_feed_server(
            Some("Lead sentence only."),
            Some("<p>Lead sentence only. More detail follows in the full article body.</p>"),
            "<html><body><article><p>Short article.</p></article></body></html>",
            None,
        )
        .await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (15, ?, 'Quality Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, 0, 0, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let result = set_single_feed_quality_overrides(&pool, 15, None, Some(false))
            .await
            .unwrap();

        assert!(!result.use_extracted_fulltext);
        assert!(!result.use_llm_summary);
        assert_eq!(result.manual_use_extracted_fulltext, None);
        assert_eq!(result.manual_use_llm_summary, Some(false));
        assert!(result.last_quality_check.is_some());
        assert!(result.last_manual_quality_override.is_some());

        let flags: (bool, bool, Option<bool>, Option<bool>, Option<i64>, Option<i64>) =
            sqlx::query_as(
                "SELECT use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary, last_quality_check, last_manual_quality_override FROM feed WHERE id = 15",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!flags.0);
        assert!(!flags.1);
        assert_eq!(flags.2, None);
        assert_eq!(flags.3, Some(false));
        assert!(flags.4.is_some());
        assert!(flags.5.is_some());
    }

    #[tokio::test]
    async fn reevaluate_single_feed_quality_clears_manual_overrides_before_recomputing() {
        let pool = setup_pool().await;
        let url = start_quality_fixture_feed_server(
            Some("Lead sentence only."),
            Some("<p>Lead sentence only. More detail follows in the full article body.</p>"),
            "<html><body><article><p>Short article.</p></article></body></html>",
            None,
        )
        .await;
        let now = unix_now();
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary, last_manual_quality_override) VALUES (16, ?, 'Quality Fixture', NULL, 1, 12345, 1, 0, 'http://example.org', 0, 0, NULL, 0, ?, 1, 0, 1, 0, ?)")
            .bind(url)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();

        let result = reevaluate_single_feed_quality(&pool, &test_config(), 16, true)
            .await
            .unwrap();

        assert!(!result.use_extracted_fulltext);
        assert!(result.use_llm_summary);
        assert_eq!(result.manual_use_extracted_fulltext, None);
        assert_eq!(result.manual_use_llm_summary, None);
        assert_eq!(result.last_manual_quality_override, None);

        let flags: (bool, bool, Option<bool>, Option<bool>, Option<i64>, Option<i64>) =
            sqlx::query_as(
                "SELECT use_extracted_fulltext, use_llm_summary, manual_use_extracted_fulltext, manual_use_llm_summary, last_manual_quality_override, next_update_time FROM feed WHERE id = 16",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!flags.0);
        assert!(flags.1);
        assert_eq!(flags.2, None);
        assert_eq!(flags.3, None);
        assert_eq!(flags.4, None);
        assert_eq!(flags.5, Some(12_345));
    }

    #[tokio::test]
    async fn reevaluate_single_feed_quality_errors_for_missing_feed() {
        let pool = setup_pool().await;

        let err = reevaluate_single_feed_quality(&pool, &test_config(), 404, true)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("feed 404 not found"));
    }

    #[tokio::test]
    async fn update_due_feeds_skips_extraction_and_summary_for_duplicate_articles() {
        let pool = setup_pool().await;
        let article_requests = Arc::new(AtomicUsize::new(0));
        let llm_requests = Arc::new(AtomicUsize::new(0));
        let url =
            start_duplicate_guard_feed_server(article_requests.clone(), llm_requests.clone()).await;
        let now = unix_now();

        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list, last_quality_check, use_extracted_fulltext, use_llm_summary) VALUES (13, ?, 'Duplicate Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0, ?, 1, 1)")
            .bind(&url)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();

        insert_test_article_with_hash(
            &pool,
            13,
            "tag:example.org,2026:duplicate-entry",
            &format!(
                "{:x}",
                md5::compute("tag:example.org,2026:duplicate-entry".as_bytes())
            ),
            now,
            1,
            0,
        )
        .await;

        let updated = update_due_feeds(
            &pool,
            &test_config_with_llm(url.trim_end_matches("/atom.xml")),
            true,
        )
        .await
        .unwrap();
        assert_eq!(updated, 1);
        assert_eq!(article_requests.load(Ordering::SeqCst), 0);
        assert_eq!(llm_requests.load(Ordering::SeqCst), 0);
    }

    async fn start_quality_fixture_feed_server(
        entry_summary: Option<&str>,
        entry_content: Option<&str>,
        article_html: &str,
        llm_response_content: Option<&str>,
    ) -> String {
        let entry_summary = entry_summary.map(str::to_string);
        let entry_content = entry_content.map(str::to_string);
        let article_html = article_html.to_string();
        let llm_response_content = llm_response_content.map(str::to_string);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let article_url = format!("http://{addr}/article");
        let app = Router::new()
            .route(
                "/article",
                get(move || {
                    let article_html = article_html.clone();
                    async move { ([(http_header::CONTENT_TYPE, "text/html")], article_html) }
                }),
            )
            .route(
                "/v1/chat/completions",
                post(move |body: String| {
                    let llm_response_content = llm_response_content.clone();
                    async move {
                        let content = if body.contains("\"name\":\"summary_quality\"") {
                            llm_response_content
                                .unwrap_or_else(|| r#"{"is_good":false}"#.to_string())
                        } else {
                            r#"{"summary":"Fixture summary from mock LLM."}"#.to_string()
                        };
                        (
                            [(http_header::CONTENT_TYPE, "application/json")],
                            json!({
                                "choices": [
                                    {
                                        "message": {
                                            "content": content
                                        }
                                    }
                                ]
                            })
                            .to_string(),
                        )
                    }
                }),
            )
            .route(
                "/atom.xml",
                get(move || {
                    let entry_summary = entry_summary.clone();
                    let entry_content = entry_content.clone();
                    let article_url = article_url.clone();
                    async move {
                        let summary_xml = entry_summary
                            .map(|summary| format!("<summary>{summary}</summary>"))
                            .unwrap_or_default();
                        let content_xml = entry_content
                            .map(|content| {
                                format!("<content type=\"html\"><![CDATA[{content}]]></content>")
                            })
                            .unwrap_or_default();
                        (
                            [(http_header::CONTENT_TYPE, "application/atom+xml")],
                            format!(
                                r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Quality Fixture</title>
  <link href="http://example.org/" />
  <updated>2026-03-06T00:00:00Z</updated>
  <id>tag:example.org,2026:feed-quality</id>
  <entry>
    <title>Quality Entry</title>
        <link href="{article_url}" />
    <id>tag:example.org,2026:quality-entry</id>
    <updated>2026-03-06T00:00:00Z</updated>
        {summary_xml}
        {content_xml}
  </entry>
</feed>"#,
                            ),
                        )
                    }
                }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/atom.xml")
    }

    async fn start_duplicate_guard_feed_server(
        article_requests: Arc<AtomicUsize>,
        llm_requests: Arc<AtomicUsize>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let article_url = format!("http://{addr}/article");
        let app = Router::new()
            .route(
                "/article",
                get(move || {
                    let article_requests = article_requests.clone();
                    async move {
                        article_requests.fetch_add(1, Ordering::SeqCst);
                        (
                            [(http_header::CONTENT_TYPE, "text/html")],
                            "<html><body><article><p>duplicate article body</p></article></body></html>",
                        )
                    }
                }),
            )
            .route(
                "/v1/chat/completions",
                post(move || {
                    let llm_requests = llm_requests.clone();
                    async move {
                        llm_requests.fetch_add(1, Ordering::SeqCst);
                        (
                            [(http_header::CONTENT_TYPE, "application/json")],
                            json!({
                                "choices": [
                                    {
                                        "message": {
                                            "content": r#"{"summary":"Should not be called."}"#
                                        }
                                    }
                                ]
                            })
                            .to_string(),
                        )
                    }
                }),
            )
            .route(
                "/atom.xml",
                get(move || {
                    let article_url = article_url.clone();
                    async move {
                        (
                            [(http_header::CONTENT_TYPE, "application/atom+xml")],
                            format!(
                                r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Duplicate Fixture</title>
  <link href="http://example.org/" />
  <updated>2026-03-06T00:00:00Z</updated>
  <id>tag:example.org,2026:duplicate-feed</id>
  <entry>
    <title>Duplicate Entry</title>
    <link href="{article_url}" />
    <id>tag:example.org,2026:duplicate-entry</id>
    <updated>2026-03-06T00:00:00Z</updated>
    <summary>Duplicate teaser</summary>
    <content type="html"><![CDATA[<p>Duplicate teaser</p>]]></content>
  </entry>
</feed>"#,
                            ),
                        )
                    }
                }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/atom.xml")
    }

    async fn insert_test_article(
        pool: &SqlitePool,
        feed_id: i64,
        guid: &str,
        last_modified: i64,
        unread: i64,
        starred: i64,
    ) {
        let guid_hash = format!("{:x}", md5::compute(guid.as_bytes()));
        insert_test_article_with_hash(
            pool,
            feed_id,
            guid,
            &guid_hash,
            last_modified,
            unread,
            starred,
        )
        .await;
    }

    async fn insert_test_article_with_hash(
        pool: &SqlitePool,
        feed_id: i64,
        guid: &str,
        guid_hash: &str,
        last_modified: i64,
        unread: i64,
        starred: i64,
    ) {
        sqlx::query(
            "INSERT INTO article (title, content, author, content_hash, enclosure_link, enclosure_mime, feed_id, fingerprint, guid, guid_hash, last_modified, media_description, media_thumbnail, pub_date, rtl, starred, unread, updated_date, url, summary) VALUES (NULL, NULL, NULL, NULL, NULL, NULL, ?, NULL, ?, ?, ?, NULL, NULL, NULL, 0, ?, ?, NULL, NULL, NULL)",
        )
        .bind(feed_id)
        .bind(guid)
        .bind(guid_hash)
        .bind(last_modified)
        .bind(starred)
        .bind(unread)
        .execute(pool)
        .await
        .unwrap();
    }
}
