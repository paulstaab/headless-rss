use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use feed_rs::model::Entry;
use rand::Rng;
use regex::Regex;
use sqlx::{FromRow, SqlitePool};

use crate::config::Config;
use crate::db;
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

#[derive(FromRow)]
struct FeedToUpdate {
    id: i64,
    url: String,
}

pub async fn update_all(config: &Config) -> Result<()> {
    tracing::info!("starting rust feed update cycle");
    let pool = db::create_pool(&config.db_path)
        .await
        .with_context(|| format!("failed to connect to sqlite db at {}", config.db_path))?;
    let updated = update_due_feeds(&pool, config.testing_mode).await?;
    tracing::info!(updated, "finished rust feed update cycle");
    Ok(())
}

pub async fn update_due_feeds(pool: &SqlitePool, testing_mode: bool) -> Result<usize> {
    let now_ts = unix_now();
    let feeds: Vec<FeedToUpdate> = sqlx::query_as(
        "SELECT id, url FROM feed WHERE is_mailing_list = 0 AND (next_update_time IS NULL OR next_update_time <= ?)",
    )
    .bind(now_ts)
    .fetch_all(pool)
    .await
    .context("failed to query due feeds")?;

    update_feed_batch(pool, testing_mode, feeds, "due").await
}

pub async fn update_all_regular_feeds(pool: &SqlitePool, testing_mode: bool) -> Result<usize> {
    let feeds: Vec<FeedToUpdate> =
        sqlx::query_as("SELECT id, url FROM feed WHERE is_mailing_list = 0")
            .fetch_all(pool)
            .await
            .context("failed to query all regular feeds")?;

    update_feed_batch(pool, testing_mode, feeds, "all").await
}

async fn update_feed_batch(
    pool: &SqlitePool,
    testing_mode: bool,
    feeds: Vec<FeedToUpdate>,
    batch_kind: &str,
) -> Result<usize> {
    let feed_http_client = http_client::build_feed_http_client()?;

    tracing::debug!(
        due_feeds = feeds.len(),
        testing_mode,
        batch_kind,
        "loaded due feeds for update"
    );

    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for feed in &feeds {
        if let Err(err) =
            update_single_feed(pool, &feed_http_client, feed.id, &feed.url, testing_mode).await
        {
            let detail = err.to_string();
            tracing::warn!(feed_id = feed.id, error = %detail, "feed update failed");
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

    Ok(feeds.len())
}

async fn update_single_feed(
    pool: &SqlitePool,
    feed_http_client: &reqwest::Client,
    feed_id: i64,
    url: &str,
    testing_mode: bool,
) -> Result<()> {
    tracing::debug!(feed_id, url, testing_mode, "starting feed update");
    ssrf::validate_remote_url(url, testing_mode).await?;

    let response = feed_http_client
        .get(url)
        .send()
        .await
        .with_context(|| format!("request failed for {url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("request failed for {url}: HTTP {}", response.status());
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read response body")?;
    let parsed = feed_rs::parser::parse(&bytes[..]).context("failed to parse feed")?;

    let mut inserted = 0usize;
    let mut processed = 0usize;
    let mut current_feed_guid_hashes = Vec::new();
    for entry in parsed.entries.iter().take(50) {
        processed += 1;
        if insert_article_from_entry(pool, feed_id, entry, &mut current_feed_guid_hashes).await? {
            inserted += 1;
        }
    }

    let removed = cleanup_stale_feed_articles(pool, feed_id, &current_feed_guid_hashes).await?;

    let now_ts = unix_now();
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

async fn cleanup_stale_feed_articles(
    pool: &SqlitePool,
    feed_id: i64,
    current_feed_guid_hashes: &[String],
) -> Result<u64> {
    let stale_before = unix_now() - NINETY_DAYS;

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
/// Policy (kept in sync with Python implementation):
/// - Sparse feeds ($\le 0.1$ articles/day): refresh roughly daily with +/-30m jitter.
/// - Active feeds: refresh at 4x observed daily rate, capped so interval is at most 12h.
fn compute_next_update_interval(avg_articles_per_day: f64, jitter_seconds: i64) -> i64 {
    if avg_articles_per_day <= 0.1 {
        return ONE_DAY + jitter_seconds.clamp(-THIRTY_MINUTES, THIRTY_MINUTES);
    }

    ((ONE_DAY as f64 / avg_articles_per_day / 4.0).round() as i64).min(TWELVE_HOURS)
}

async fn insert_article_from_entry(
    pool: &SqlitePool,
    feed_id: i64,
    entry: &Entry,
    current_feed_guid_hashes: &mut Vec<String>,
) -> Result<bool> {
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
        tracing::debug!(feed_id, "skipping entry without guid/link/title");
        return Ok(false);
    };

    let guid_hash = format!("{:x}", md5::compute(guid.as_bytes()));
    current_feed_guid_hashes.push(guid_hash.clone());
    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM article WHERE guid_hash = ? LIMIT 1")
            .bind(&guid_hash)
            .fetch_optional(pool)
            .await
            .context("failed to check existing article")?;

    if existing.is_some() {
        tracing::debug!(feed_id, guid_hash, "skipping duplicate entry");
        return Ok(false);
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
    .context("failed to insert article")?;

    Ok(true)
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

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::http::header as http_header;
    use axum::http::{HeaderMap as AxumHeaderMap, StatusCode};
    use axum::routing::get;
    use sqlx::SqlitePool;
    use tokio::net::TcpListener;

    use super::{
        NINETY_DAYS, ONE_DAY, THIRTY_MINUTES, TWELVE_HOURS, compute_next_update_interval,
        unix_now,
    };
    use super::{update_all_regular_feeds, update_due_feeds};

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE feed (id INTEGER PRIMARY KEY NOT NULL, url VARCHAR NOT NULL UNIQUE, title VARCHAR, favicon_link VARCHAR, added INTEGER NOT NULL, next_update_time INTEGER, folder_id INTEGER NOT NULL, ordering INTEGER NOT NULL, link VARCHAR, pinned BOOLEAN NOT NULL, update_error_count INTEGER NOT NULL, last_update_error VARCHAR, is_mailing_list BOOLEAN NOT NULL DEFAULT 0)",
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
        pool
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (1, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let now_before = unix_now();
        let updated = update_due_feeds(&pool, true).await.unwrap();
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
        let min_expected = now_before + TWELVE_HOURS - 2;
        let max_expected = now_after + TWELVE_HOURS + 2;
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (4, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, true).await.unwrap();
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (5, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, true).await.unwrap();
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (2, 'file:///etc/passwd', 'Bad', NULL, 1, 0, 1, 0, NULL, 0, 0, NULL, 0)")
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, false).await.unwrap();
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
                .contains("Only http and https are permitted")
        );
    }

    #[tokio::test]
    async fn update_due_feeds_skips_mailing_list_rows() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (3, 'newsletter@example.com', 'News', NULL, 1, 0, 1, 0, NULL, 0, 0, NULL, 1)")
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_due_feeds(&pool, false).await.unwrap();
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (6, ?, 'Updater Fixture', NULL, 1, 9999999999, 1, 0, 'http://example.org', 0, 0, NULL, 0)")
            .bind(url)
            .execute(&pool)
            .await
            .unwrap();

        let updated = update_all_regular_feeds(&pool, true).await.unwrap();
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
        sqlx::query("INSERT INTO feed (id, url, title, favicon_link, added, next_update_time, folder_id, ordering, link, pinned, update_error_count, last_update_error, is_mailing_list) VALUES (7, ?, 'Updater Fixture', NULL, 1, 0, 1, 0, 'http://example.org', 0, 0, NULL, 0)")
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
        insert_test_article_with_hash(&pool, 7, in_payload_guid, &in_payload_hash, stale, 0, 0).await;

        let updated = update_due_feeds(&pool, true).await.unwrap();
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

        let fresh_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM article WHERE guid = 'fresh-read-unstarred' LIMIT 1")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert!(fresh_exists.is_some());

        let in_payload_exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM article WHERE guid_hash = ? LIMIT 1",
        )
        .bind(in_payload_hash)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(in_payload_exists.is_some());
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
        insert_test_article_with_hash(pool, feed_id, guid, &guid_hash, last_modified, unread, starred).await;
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
