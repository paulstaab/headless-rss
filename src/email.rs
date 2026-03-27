//! Newsletter ingestion for IMAP-backed mailing lists.
//!
//! This module is responsible for loading unread mailbox messages, filtering them to
//! mailing-list/newsletter traffic, converting those messages into article rows, and
//! applying newsletter-specific cleanup behavior.

use std::collections::HashSet;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use mailparse::{MailHeaderMap, ParsedMail};
use native_tls::TlsConnector;
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{FromRow, SqlitePool};
use std::env;
use std::fs;

use crate::article_store::{self, ArticleIngestionContext, ArticleRecord, InsertArticleOutcome};
use crate::config::Config;
use crate::content;
use crate::http_client;
use crate::llm;
use crate::llm::LlmRequestContext;
use crate::repo;

const NINETY_DAYS: i64 = 90 * 24 * 60 * 60;
const NEWSLETTER_MAX_CHARS: usize = 5_000;
const NEWSLETTER_MAX_ITEMS: usize = 25;
const NEWSLETTER_SINGLE_SUMMARY_MAX_CHARS: usize = 160;

#[derive(Clone, Debug, FromRow)]
struct EmailCredentialRow {
    protocol: String,
    server: String,
    port: i64,
    username: String,
    password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NewsletterLlmResult {
    mode: String,
    summary: Option<String>,
    content: Option<String>,
    items: Vec<NewsletterItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NewsletterItem {
    title: Option<String>,
    url: String,
    summary: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct RawNewsletterLlmResult {
    #[allow(dead_code)]
    mode: String,
    summary: Option<String>,
    content: Option<String>,
    items: Option<Vec<RawNewsletterItem>>,
}

#[derive(Deserialize)]
struct RawNewsletterItem {
    title: Option<String>,
    url: Option<String>,
    summary: Option<String>,
    content: Option<String>,
}

/// Fetch unread mailing-list messages from configured mailboxes and persist them as newsletter items.
pub async fn fetch_emails_from_all_mailboxes(pool: &SqlitePool, config: &Config) -> Result<()> {
    fetch_emails_from_all_mailboxes_with_fetcher(pool, config, fetch_unread_messages_blocking).await
}

async fn fetch_emails_from_all_mailboxes_with_fetcher<F>(
    pool: &SqlitePool,
    config: &Config,
    fetcher: F,
) -> Result<()>
where
    F: Fn(&EmailCredentialRow) -> Result<Vec<Vec<u8>>>,
{
    let credentials: Vec<EmailCredentialRow> = sqlx::query_as(
        "SELECT protocol, server, port, username, password FROM email_credentials ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .context("failed to load email credentials")?;

    if credentials.is_empty() {
        tracing::info!("no email credentials configured; skipping mailbox fetch");
        clean_up_old_newsletters(pool, None).await?;
        return Ok(());
    }

    let article_http_client = http_client::build_article_http_client()?;

    for credential in credentials {
        match fetcher(&credential) {
            Ok(raw_messages) => {
                tracing::info!(count = raw_messages.len(), "fetched unread email messages");

                for raw_message in raw_messages {
                    if let Err(err) =
                        process_email_message(pool, &article_http_client, config, &raw_message)
                            .await
                    {
                        let _ = err;
                        tracing::warn!("failed to process newsletter email");
                    }
                }
            }
            Err(err) => {
                let _ = err;
                tracing::warn!("failed to fetch unread emails from mailbox");
            }
        }
    }

    clean_up_old_newsletters(pool, None).await?;
    Ok(())
}

/// Remove stale newsletter entries that are older than 90 days, read, and unstarred.
pub async fn clean_up_old_newsletters(pool: &SqlitePool, now_ts: Option<i64>) -> Result<u64> {
    let cutoff = now_ts.unwrap_or_else(article_store::unix_now) - NINETY_DAYS;
    let result = sqlx::query(
        "DELETE FROM article WHERE feed_id IN (SELECT id FROM feed WHERE is_mailing_list = 1) AND last_modified < ? AND unread = 0 AND starred = 0",
    )
    .bind(cutoff)
    .execute(pool)
    .await
    .context("failed to clean up stale newsletter articles")?;

    Ok(result.rows_affected())
}

/// Parses one raw email, discards non-newsletters, and inserts any resulting article drafts.
async fn process_email_message(
    pool: &SqlitePool,
    article_http_client: &reqwest::Client,
    config: &Config,
    raw_email: &[u8],
) -> Result<usize> {
    let parsed = mailparse::parse_mail(raw_email).context("failed to parse raw email")?;

    if !is_mailing_list(&parsed) {
        return Ok(0);
    }

    let subject = extract_subject(&parsed);
    let from_header = parsed.headers.get_first_value("From").unwrap_or_default();
    let from_address = extract_sender_address(&from_header);
    if from_address.is_empty() {
        return Ok(0);
    }

    let feed_title = extract_feed_title(&from_header, &from_address);
    let feed_id = find_or_create_mailing_list_feed(pool, &from_address, &feed_title).await?;
    let raw_content = extract_message_content(&parsed);
    let cleaned_content = if looks_like_html(&raw_content) {
        clean_newsletter_html(&raw_content)
    } else {
        raw_content
    };

    let llm_result =
        parse_newsletter_with_llm(config, feed_id, &subject, &from_address, &cleaned_content).await;
    let articles = build_articles_from_email(
        feed_id,
        &subject,
        &from_address,
        &cleaned_content,
        llm_result,
    );

    let mut inserted = 0usize;
    let ingestion = ArticleIngestionContext {
        pool,
        article_http_client,
        config,
        feed_url: None,
        content_state: content::FeedContentState {
            last_quality_check: None,
            use_extracted_fulltext: false,
            use_llm_summary: false,
            manual_use_extracted_fulltext: None,
            manual_use_llm_summary: None,
        },
    };
    for article in articles {
        match article_store::ingest_article_if_new(&ingestion, article)
            .await
            .context("failed to insert newsletter article")?
        {
            InsertArticleOutcome::Inserted { .. } => inserted += 1,
            InsertArticleOutcome::Duplicate { .. } => {}
        }
    }

    clear_mailing_list_feed_error_state(pool, feed_id).await?;

    Ok(inserted)
}

/// Builds one or more article drafts from a cleaned newsletter body and optional LLM parse result.
fn build_articles_from_email(
    feed_id: i64,
    subject: &str,
    from_address: &str,
    content: &str,
    llm_result: Option<NewsletterLlmResult>,
) -> Vec<ArticleRecord> {
    match llm_result {
        Some(result) if result.mode == "multi" => {
            let mut articles = Vec::new();
            for item in result.items.into_iter().take(NEWSLETTER_MAX_ITEMS) {
                let title = item.title.unwrap_or_else(|| subject.to_string());
                let item_content = item
                    .content
                    .or_else(|| item.summary.clone())
                    .unwrap_or_default();
                articles.push(build_newsletter_article_record(
                    feed_id,
                    &title,
                    from_address,
                    &item_content,
                    item.summary,
                    Some(item.url),
                ));
            }

            if !articles.is_empty() {
                return articles;
            }

            vec![build_newsletter_article_record(
                feed_id,
                subject,
                from_address,
                content,
                result.summary,
                None,
            )]
        }
        Some(result) => {
            let article_content = result.content.unwrap_or_else(|| content.to_string());
            vec![build_newsletter_article_record(
                feed_id,
                subject,
                from_address,
                &article_content,
                result.summary,
                None,
            )]
        }
        None => vec![build_newsletter_article_record(
            feed_id,
            subject,
            from_address,
            content,
            None,
            None,
        )],
    }
}

/// Builds a newsletter-derived article record before shared enrichment and insertion.
fn build_newsletter_article_record(
    feed_id: i64,
    subject: &str,
    from_address: &str,
    content: &str,
    summary: Option<String>,
    url: Option<String>,
) -> ArticleRecord {
    let now_ts = article_store::unix_now();
    let guid = match url.as_deref() {
        Some(url) => format!("{from_address}:{subject}:{url}"),
        None => format!("{from_address}:{subject}"),
    };

    ArticleRecord {
        title: Some(subject.to_string()),
        author: Some(from_address.to_string()),
        content: Some(content.to_string()),
        summary,
        content_hash: None,
        feed_id,
        guid_hash: article_store::guid_hash(&guid),
        guid,
        last_modified: now_ts,
        media_thumbnail: None,
        pub_date: Some(now_ts),
        updated_date: Some(now_ts),
        url,
        starred: false,
        unread: true,
    }
}

/// Returns an existing mailing-list feed for a sender address or creates one under the root folder.
async fn find_or_create_mailing_list_feed(
    pool: &SqlitePool,
    from_address: &str,
    feed_title: &str,
) -> Result<i64> {
    let existing_id: Option<i64> = sqlx::query_scalar("SELECT id FROM feed WHERE url = ? LIMIT 1")
        .bind(from_address)
        .fetch_optional(pool)
        .await
        .context("failed to query mailing-list feed")?;

    if let Some(feed_id) = existing_id {
        return Ok(feed_id);
    }

    let root_id = root_folder_id(pool).await?;
    repo::create_mailing_list_feed(
        pool,
        from_address,
        feed_title,
        root_id,
        article_store::unix_now(),
    )
    .await
    .context("failed to create mailing-list feed")
}

/// Clears stale refresh error metadata after a mailing-list message is processed successfully.
async fn clear_mailing_list_feed_error_state(pool: &SqlitePool, feed_id: i64) -> Result<()> {
    sqlx::query(
        "UPDATE feed SET update_error_count = 0, last_update_error = NULL WHERE id = ? AND is_mailing_list = 1",
    )
    .bind(feed_id)
    .execute(pool)
    .await
    .context("failed to clear mailing-list feed update error state")?;

    Ok(())
}

/// Resolves the internal root folder, creating it if a fresh test database does not contain one.
async fn root_folder_id(pool: &SqlitePool) -> Result<i64> {
    repo::get_root_folder_id(pool)
        .await
        .context("failed to query root folder")
}

/// Loads unread mailbox messages either from the test hook or from the configured IMAP server.
fn fetch_unread_messages_blocking(credential: &EmailCredentialRow) -> Result<Vec<Vec<u8>>> {
    if let Some(mock_messages) = load_mock_messages_from_env()? {
        return Ok(mock_messages);
    }

    if credential.protocol != "imap" {
        anyhow::bail!("unsupported email protocol: {}", credential.protocol);
    }

    let tls = TlsConnector::builder()
        .build()
        .context("failed to initialize tls connector")?;
    let client = imap::connect(
        (&*credential.server, credential.port as u16),
        &credential.server,
        &tls,
    )
    .with_context(|| {
        format!(
            "failed to connect to mailbox at {}:{}",
            credential.server, credential.port
        )
    })?;
    let mut session = client
        .login(&credential.username, &credential.password)
        .map_err(|(error, _client)| error)
        .with_context(|| {
            format!(
                "failed to login to mailbox at {}:{}",
                credential.server, credential.port
            )
        })?;
    session.select("inbox").context("failed to select inbox")?;

    let search = session
        .search("UNSEEN")
        .context("failed to search unseen emails")?;
    let mut messages = Vec::new();
    for message_id in search.iter() {
        let fetches = session
            .fetch(message_id.to_string(), "RFC822")
            .with_context(|| format!("failed to fetch email id {message_id}"))?;
        for fetched in fetches.iter() {
            if let Some(body) = fetched.body() {
                messages.push(body.to_vec());
            }
        }
        session
            .store(message_id.to_string(), "+FLAGS (\\Seen)")
            .with_context(|| format!("failed to mark email id {message_id} as seen"))?;
    }

    session.logout().context("failed to logout from mailbox")?;
    Ok(messages)
}

/// Reads serialized mock IMAP messages from the test-only environment hook.
fn load_mock_messages_from_env() -> Result<Option<Vec<Vec<u8>>>> {
    let testing_mode = env::var("TESTING_MODE")
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if !testing_mode {
        return Ok(None);
    }

    let Some(path) = env::var("HEADLESS_RSS_TEST_IMAP_MESSAGES_FILE").ok() else {
        return Ok(None);
    };

    let file_content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read mock IMAP message file at {path}"))?;
    let messages: Vec<String> = serde_json::from_str(&file_content)
        .with_context(|| format!("failed to parse mock IMAP message file at {path}"))?;

    Ok(Some(
        messages
            .into_iter()
            .map(|message| message.into_bytes())
            .collect(),
    ))
}

/// Extracts the decoded message subject from parsed mail headers.
fn extract_subject(parsed: &ParsedMail<'_>) -> String {
    parsed
        .headers
        .get_first_value("Subject")
        .unwrap_or_default()
}

/// Extracts the sender email address from a `From` header.
fn extract_sender_address(from_header: &str) -> String {
    if let Some(start) = from_header.find('<')
        && let Some(end) = from_header[start + 1..].find('>')
    {
        return from_header[start + 1..start + 1 + end].trim().to_string();
    }

    from_header.trim().to_string()
}

/// Derives a human-readable feed title from the `From` header or sender domain.
fn extract_feed_title(from_header: &str, from_address: &str) -> String {
    if let Some(start) = from_header.find('<') {
        return from_header[..start].trim().to_string();
    }

    from_address
        .split('@')
        .nth(1)
        .and_then(|domain| domain.split('.').next())
        .unwrap_or(from_address)
        .to_string()
}

/// Returns true when the message exposes mailing-list metadata expected for newsletters.
fn is_mailing_list(parsed: &ParsedMail<'_>) -> bool {
    parsed.headers.get_first_value("List-Unsubscribe").is_some()
}

/// Selects the best available newsletter body, preferring HTML over plain text when present.
fn extract_message_content(parsed: &ParsedMail<'_>) -> String {
    if let Some(html_part) = find_message_part(parsed, "text/html") {
        return html_part.get_body().unwrap_or_default();
    }

    if let Some(text_part) = find_message_part(parsed, "text/plain") {
        return text_part.get_body().unwrap_or_default();
    }

    parsed.get_body().unwrap_or_default()
}

/// Recursively finds the first non-attachment MIME part matching the requested content type.
fn find_message_part<'a>(
    parsed: &'a ParsedMail<'a>,
    mime_type: &str,
) -> Option<&'a ParsedMail<'a>> {
    if parsed.ctype.mimetype.eq_ignore_ascii_case(mime_type) && !is_attachment(parsed) {
        return Some(parsed);
    }

    for subpart in &parsed.subparts {
        if let Some(found) = find_message_part(subpart, mime_type) {
            return Some(found);
        }
    }

    None
}

/// Returns true when the MIME part is marked as an attachment.
fn is_attachment(parsed: &ParsedMail<'_>) -> bool {
    parsed
        .headers
        .get_first_value("Content-Disposition")
        .map(|value| value.to_ascii_lowercase().contains("attachment"))
        .unwrap_or(false)
}

/// Heuristically determines whether a message body should be treated as HTML.
fn looks_like_html(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with('<') || trimmed.contains("<html") || trimmed.contains("<body")
}

/// Strips common newsletter clutter such as hidden blocks, layout tables, and tracking pixels.
fn clean_newsletter_html(html_content: &str) -> String {
    if html_content.trim().is_empty() {
        return String::new();
    }

    let without_tracking_pixels = tracking_pixel_regex().replace_all(html_content, "");
    let without_meta = meta_tag_regex().replace_all(&without_tracking_pixels, "");
    let without_hidden = hidden_div_regex().replace_all(&without_meta, "");
    let simplified_tables = table_tag_regex().replace_all(&without_hidden, "");
    let simplified_cells = table_cell_tag_regex().replace_all(&simplified_tables, "");
    let collapsed = whitespace_regex().replace_all(&simplified_cells, " ");
    collapsed.trim().to_string()
}

/// Requests an optional newsletter structure parse from the configured OpenAI-compatible endpoint.
async fn parse_newsletter_with_llm(
    config: &Config,
    feed_id: i64,
    subject: &str,
    from_address: &str,
    content: &str,
) -> Option<NewsletterLlmResult> {
    config.openai_api_key.as_deref()?;

    let trimmed_content = truncate_chars(content, NEWSLETTER_MAX_CHARS);
    if trimmed_content.trim().is_empty() {
        return None;
    }

    let response_text = llm::request_chat_completion_content(
        config,
        build_openai_newsletter_payload(
            &config.openai_model,
            subject,
            from_address,
            &trimmed_content,
        ),
        LlmRequestContext {
            task_name: "newsletter-parsing",
            feed_id: Some(feed_id),
            article_id: None,
        },
    )
    .await?;

    let parsed = match parse_newsletter_llm_json_response(&response_text) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::error!(error = %err, feed_id, "newsletter LLM response could not be parsed");
            return None;
        }
    };

    Some(normalize_llm_result(parsed, content))
}

/// Parses a structured newsletter LLM response, accepting either top-level fields
/// or a schema-name wrapper under `newsletter_parse`.
fn parse_newsletter_llm_json_response(response_text: &str) -> Result<RawNewsletterLlmResult> {
    let parsed: Value = serde_json::from_str(response_text)
        .context("newsletter LLM response was not valid JSON")?;

    if let Ok(result) = serde_json::from_value::<RawNewsletterLlmResult>(parsed.clone()) {
        return Ok(result);
    }

    let wrapped = parsed
        .get("newsletter_parse")
        .cloned()
        .context("newsletter LLM response did not include expected fields")?;

    tracing::info!("unwrapped structured newsletter LLM response");

    serde_json::from_value(wrapped)
        .context("newsletter LLM response wrapper did not match the expected schema")
}

/// Normalizes raw LLM output into a deterministic newsletter parse result.
///
/// Multi-item mode is used only when the parse yields at least two distinct linked
/// items. All other cases are coerced into single-item mode with fallback content
/// and a concise fallback summary so downstream ingestion always has a usable shape.
fn normalize_llm_result(
    result: RawNewsletterLlmResult,
    fallback_content: &str,
) -> NewsletterLlmResult {
    let mut seen_urls = HashSet::new();
    let items: Vec<NewsletterItem> = result
        .items
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let url = normalize_optional_text(item.url)?;
            if !seen_urls.insert(url.clone()) {
                return None;
            }

            Some(NewsletterItem {
                title: normalize_optional_text(item.title),
                url,
                summary: normalize_optional_text(item.summary),
                content: normalize_optional_text(item.content),
            })
        })
        .collect();

    let summary = normalize_optional_text(result.summary);
    let content = normalize_optional_text(result.content)
        .or_else(|| normalize_optional_text(Some(fallback_content.to_string())));

    if items.len() >= 2 {
        return NewsletterLlmResult {
            mode: "multi".to_string(),
            summary,
            content,
            items,
        };
    }

    NewsletterLlmResult {
        mode: "single".to_string(),
        summary: summary.or_else(|| content.as_deref().and_then(build_single_mode_summary)),
        content,
        items: Vec::new(),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn build_single_mode_summary(content: &str) -> Option<String> {
    let plain = newsletter_plain_text(content);
    if plain.is_empty() {
        return None;
    }

    if plain.chars().count() <= NEWSLETTER_SINGLE_SUMMARY_MAX_CHARS {
        return Some(plain);
    }

    Some(format!(
        "{}...",
        truncate_chars(&plain, NEWSLETTER_SINGLE_SUMMARY_MAX_CHARS)
    ))
}

fn newsletter_plain_text(content: &str) -> String {
    let without_tags = html_tag_regex().replace_all(content, " ");
    whitespace_regex()
        .replace_all(&without_tags, " ")
        .trim()
        .to_string()
}

/// Builds the structured-output payload used for newsletter parsing requests.
fn build_openai_newsletter_payload(
    model: &str,
    subject: &str,
    from_address: &str,
    content: &str,
) -> serde_json::Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are parsing a newsletter email. Decide if it is a list of distinct article links with short descriptions. If yes, return mode=multi with an ordered list of items. If no, return mode=single with a cleaned content field and a concise summary. Always keep formatting minimal: paragraphs, lists, and links only."
            },
            {
                "role": "user",
                "content": format!("Subject: {subject}\nFrom: {from_address}\n\nContent:\n{content}")
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "newsletter_parse",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "mode": {"type": "string", "enum": ["single", "multi"]},
                        "summary": {"type": "string"},
                        "content": {"type": "string"},
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "title": {"type": "string"},
                                    "url": {"type": "string"},
                                    "summary": {"type": "string"},
                                    "content": {"type": "string"}
                                },
                                "required": ["url"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["mode"],
                    "additionalProperties": false
                }
            }
        }
    })
}

/// Returns the compiled regex used to remove 1px tracking images and similar beacons.
fn tracking_pixel_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?is)<img[^>]*(?:width\s*=\s*["']?1["']?|height\s*=\s*["']?1["']?|style\s*=\s*["'][^"']*display\s*:\s*none[^"']*["']|src\s*=\s*["'][^"']*(?:tracking|pixel)[^"']*["'])[^>]*>"#)
            .expect("valid tracking pixel regex")
    })
}

/// Returns the compiled regex used to strip `<meta>` tags from newsletter HTML.
fn meta_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<meta[^>]*>").expect("valid meta tag regex"))
}

/// Returns the compiled regex used to remove hidden div sections from newsletter HTML.
fn hidden_div_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?is)<div[^>]*style\s*=\s*["'][^"']*display\s*:\s*none[^"']*["'][^>]*>.*?</div>"#,
        )
        .expect("valid hidden div regex")
    })
}

/// Returns the compiled regex used to remove layout-table wrappers.
fn table_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)</?table[^>]*>").expect("valid table tag regex"))
}

/// Returns the compiled regex used to remove table cell/container tags after layout flattening.
fn table_cell_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?is)</?(?:tbody|tr|td|th)[^>]*>").expect("valid table cell tag regex")
    })
}

/// Returns the compiled regex used to collapse repeated whitespace.
fn whitespace_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"))
}

/// Returns the compiled regex used to strip HTML tags when building plain-text fallbacks.
fn html_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?is)<[^>]+>").expect("valid html strip regex"))
}

/// Truncates text by character count while preserving valid UTF-8 boundaries.
fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::config::Config;

    use super::{
        EmailCredentialRow, NEWSLETTER_MAX_ITEMS, NEWSLETTER_SINGLE_SUMMARY_MAX_CHARS,
        NewsletterItem, NewsletterLlmResult, build_articles_from_email, clean_newsletter_html,
        clean_up_old_newsletters, fetch_emails_from_all_mailboxes_with_fetcher,
        load_mock_messages_from_env, normalize_llm_result, parse_newsletter_llm_json_response,
        process_email_message,
    };

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE folder (id INTEGER PRIMARY KEY NOT NULL, name VARCHAR NOT NULL UNIQUE, is_root BOOLEAN NOT NULL DEFAULT 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE feed (id INTEGER PRIMARY KEY NOT NULL, url VARCHAR NOT NULL UNIQUE, title VARCHAR, favicon_link VARCHAR, added INTEGER NOT NULL, next_update_time INTEGER, folder_id INTEGER NOT NULL, ordering INTEGER NOT NULL DEFAULT 0, link VARCHAR, pinned BOOLEAN NOT NULL DEFAULT 0, update_error_count INTEGER NOT NULL DEFAULT 0, last_update_error VARCHAR, is_mailing_list BOOLEAN NOT NULL DEFAULT 0, last_quality_check INTEGER, use_extracted_fulltext BOOLEAN NOT NULL DEFAULT 0, use_llm_summary BOOLEAN NOT NULL DEFAULT 0, manual_use_extracted_fulltext BOOLEAN, manual_use_llm_summary BOOLEAN, last_manual_quality_override INTEGER)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE article (id INTEGER PRIMARY KEY NOT NULL, title VARCHAR, content VARCHAR, author VARCHAR, summary VARCHAR, content_hash VARCHAR, enclosure_link VARCHAR, enclosure_mime VARCHAR, feed_id INTEGER NOT NULL, fingerprint VARCHAR, guid VARCHAR NOT NULL, guid_hash VARCHAR NOT NULL, last_modified INTEGER NOT NULL, media_description VARCHAR, media_thumbnail VARCHAR, pub_date INTEGER, rtl BOOLEAN NOT NULL DEFAULT 0, starred BOOLEAN NOT NULL DEFAULT 0, unread BOOLEAN NOT NULL DEFAULT 1, updated_date INTEGER, url VARCHAR)",
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
        sqlx::query("INSERT INTO folder (id, name, is_root) VALUES (0, '', 1)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn config() -> Config {
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

    #[tokio::test]
    async fn mailbox_ingestion_creates_mailing_list_feeds_and_deduplicates_articles() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO email_credentials (protocol, server, port, username, password) VALUES ('imap', 'imap.example.com', 993, 'user@example.com', 'secret')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let messages = vec![
            b"Subject: Test Email 1\r\nFrom: Example List <list1@example.com>\r\nList-Unsubscribe: <mailto:unsubscribe@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody 1".to_vec(),
            b"Subject: Test Email 2\r\nFrom: Example List <list1@example.com>\r\nList-Unsubscribe: <mailto:unsubscribe@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody 2".to_vec(),
            b"Subject: Ignore Me\r\nFrom: Personal <person@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nNot a mailing list".to_vec(),
            b"Subject: Test Email 3\r\nFrom: Another List <list2@example.com>\r\nList-Unsubscribe: <mailto:unsubscribe@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody 3".to_vec(),
        ];

        fetch_emails_from_all_mailboxes_with_fetcher(&pool, &config(), |_credential| {
            Ok(messages.clone())
        })
        .await
        .unwrap();
        fetch_emails_from_all_mailboxes_with_fetcher(&pool, &config(), |_credential| {
            Ok(messages.clone())
        })
        .await
        .unwrap();

        let feed_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM feed WHERE is_mailing_list = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(feed_count, 2);

        let list1_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM article WHERE feed_id = (SELECT id FROM feed WHERE url = 'list1@example.com')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let list2_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM article WHERE feed_id = (SELECT id FROM feed WHERE url = 'list2@example.com')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(list1_count, 2);
        assert_eq!(list2_count, 1);
    }

    #[tokio::test]
    async fn html_newsletter_content_is_cleaned_before_persistence() {
        let pool = setup_pool().await;
        let raw_email = b"Subject: HTML Newsletter\r\nFrom: Example List <list@example.com>\r\nList-Unsubscribe: <mailto:unsubscribe@example.com>\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><head><meta charset=\"utf-8\"></head><body><div style=\"display:none\">hidden text</div><table border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><tr><td><p>Visible content</p><img width=\"1\" height=\"1\" src=\"https://example.com/tracking.gif\"></td></tr></table></body></html>";

        let client = reqwest::Client::new();
        process_email_message(&pool, &client, &config(), raw_email)
            .await
            .unwrap();

        let content: String = sqlx::query_scalar("SELECT content FROM article LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(content.contains("Visible content"));
        assert!(!content.contains("hidden text"));
        assert!(!content.contains("tracking.gif"));
        assert!(!content.contains("<meta"));
    }

    #[tokio::test]
    async fn successful_newsletter_processing_clears_stale_feed_error_state() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO feed (id, url, title, added, folder_id, ordering, pinned, update_error_count, last_update_error, is_mailing_list, use_extracted_fulltext, use_llm_summary) VALUES (1, 'list@example.com', 'Example List', 1, 0, 0, 0, 2, 'invalid url', 1, 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let raw_email = b"Subject: Recovery Newsletter\r\nFrom: Example List <list@example.com>\r\nList-Unsubscribe: <mailto:unsubscribe@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBody";
        let client = reqwest::Client::new();
        process_email_message(&pool, &client, &config(), raw_email)
            .await
            .unwrap();

        let err_count: i64 = sqlx::query_scalar("SELECT update_error_count FROM feed WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let err_detail: Option<String> =
            sqlx::query_scalar("SELECT last_update_error FROM feed WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(err_count, 0);
        assert_eq!(err_detail, None);
    }

    #[tokio::test]
    async fn llm_multi_mode_builds_separate_articles_and_caps_to_25_items() {
        let items = (0..(NEWSLETTER_MAX_ITEMS + 5))
            .map(|index| NewsletterItem {
                title: Some(format!("Item {index}")),
                url: format!("https://example.com/{index}"),
                summary: Some(format!("Summary {index}")),
                content: None,
            })
            .collect();
        let result = NewsletterLlmResult {
            mode: "multi".to_string(),
            summary: None,
            content: None,
            items,
        };

        let articles = build_articles_from_email(
            1,
            "Newsletter",
            "list@example.com",
            "fallback body",
            Some(result),
        );

        assert_eq!(articles.len(), NEWSLETTER_MAX_ITEMS);
        assert_eq!(articles[0].url.as_deref(), Some("https://example.com/0"));
        assert_eq!(articles[0].summary.as_deref(), Some("Summary 0"));
    }

    #[tokio::test]
    async fn llm_single_mode_builds_one_article_with_clean_content() {
        let result = NewsletterLlmResult {
            mode: "single".to_string(),
            summary: Some("Concise summary".to_string()),
            content: Some("Cleaned content text".to_string()),
            items: Vec::new(),
        };

        let articles = build_articles_from_email(
            1,
            "Newsletter",
            "list@example.com",
            "fallback body",
            Some(result),
        );

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].content.as_deref(), Some("Cleaned content text"));
        assert_eq!(articles[0].summary.as_deref(), Some("Concise summary"));
        assert!(articles[0].url.is_none());
    }

    #[test]
    fn newsletter_parser_accepts_schema_wrapped_response() {
        let parsed = parse_newsletter_llm_json_response(
            r#"{"newsletter_parse":{"mode":"multi","items":[{"url":"https://example.com/1","summary":"One"},{"url":"https://example.com/2","summary":"Two"}]}}"#,
        )
        .unwrap();

        assert_eq!(parsed.mode, "multi");
        assert_eq!(parsed.items.as_ref().map(Vec::len), Some(2));
    }

    #[tokio::test]
    async fn cleanup_only_removes_stale_read_unstarred_newsletters() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO feed (id, url, title, added, folder_id, ordering, pinned, update_error_count, is_mailing_list, use_extracted_fulltext, use_llm_summary) VALUES (1, 'list@example.com', 'Example List', 1, 0, 0, 0, 0, 1, 0, 0), (2, 'https://example.com/rss', 'Regular Feed', 1, 0, 0, 0, 0, 0, 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let base_time = 1_000_000_i64;
        let old = base_time - (91 * 24 * 60 * 60);
        sqlx::query(
            "INSERT INTO article (id, title, feed_id, guid, guid_hash, last_modified, pub_date, unread, starred, updated_date, content) VALUES (1, 'old-read', 1, 'old-read', 'hash-1', ?, ?, 0, 0, ?, 'content')",
        )
        .bind(old)
        .bind(old)
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO article (id, title, feed_id, guid, guid_hash, last_modified, pub_date, unread, starred, updated_date, content) VALUES (2, 'old-unread', 1, 'old-unread', 'hash-2', ?, ?, 1, 0, ?, 'content')",
        )
        .bind(old)
        .bind(old)
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO article (id, title, feed_id, guid, guid_hash, last_modified, pub_date, unread, starred, updated_date, content) VALUES (3, 'old-starred', 1, 'old-starred', 'hash-3', ?, ?, 0, 1, ?, 'content')",
        )
        .bind(old)
        .bind(old)
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO article (id, title, feed_id, guid, guid_hash, last_modified, pub_date, unread, starred, updated_date, content) VALUES (4, 'recent', 1, 'recent', 'hash-4', ?, ?, 0, 0, ?, 'content')",
        )
        .bind(base_time)
        .bind(base_time)
        .bind(base_time)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO article (id, title, feed_id, guid, guid_hash, last_modified, pub_date, unread, starred, updated_date, content) VALUES (5, 'regular-old', 2, 'regular-old', 'hash-5', ?, ?, 0, 0, ?, 'content')",
        )
        .bind(old)
        .bind(old)
        .bind(old)
        .execute(&pool)
        .await
        .unwrap();

        let removed = clean_up_old_newsletters(&pool, Some(base_time))
            .await
            .unwrap();
        assert_eq!(removed, 1);

        let remaining: Vec<String> = sqlx::query_scalar("SELECT guid FROM article ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(!remaining.contains(&"old-read".to_string()));
        assert!(remaining.contains(&"old-unread".to_string()));
        assert!(remaining.contains(&"old-starred".to_string()));
        assert!(remaining.contains(&"recent".to_string()));
        assert!(remaining.contains(&"regular-old".to_string()));
    }

    #[tokio::test]
    async fn no_credentials_still_runs_cleanup_successfully() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO feed (id, url, title, added, folder_id, ordering, pinned, update_error_count, is_mailing_list, use_extracted_fulltext, use_llm_summary) VALUES (1, 'list@example.com', 'Example List', 1, 0, 0, 0, 0, 1, 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO article (id, title, feed_id, guid, guid_hash, last_modified, pub_date, unread, starred, updated_date, content) VALUES (1, 'old-read', 1, 'old-read', 'hash-1', 1, 1, 0, 0, 1, 'content')",
        )
        .execute(&pool)
        .await
        .unwrap();

        fetch_emails_from_all_mailboxes_with_fetcher(
            &pool,
            &config(),
            |_credential: &EmailCredentialRow| {
                panic!("fetcher should not be called without credentials")
            },
        )
        .await
        .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM article")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn html_cleanup_strips_hidden_and_tracking_markup() {
        let cleaned = clean_newsletter_html(
            r#"<html><meta charset="utf-8"><body><div style="display:none">hidden</div><table><tr><td><p>hello</p><img width="1" height="1" src="https://example.com/pixel.gif"></td></tr></table></body></html>"#,
        );
        assert!(cleaned.contains("hello"));
        assert!(!cleaned.contains("hidden"));
        assert!(!cleaned.contains("pixel.gif"));
    }

    #[test]
    fn normalize_llm_result_discards_items_without_urls() {
        let normalized = normalize_llm_result(
            super::RawNewsletterLlmResult {
                mode: "multi".to_string(),
                summary: None,
                content: None,
                items: Some(vec![
                    super::RawNewsletterItem {
                        title: Some("keep".to_string()),
                        url: Some("https://example.com/1".to_string()),
                        summary: Some("summary".to_string()),
                        content: None,
                    },
                    super::RawNewsletterItem {
                        title: Some("drop".to_string()),
                        url: None,
                        summary: None,
                        content: None,
                    },
                ]),
            },
            "Fallback newsletter body",
        );

        assert_eq!(normalized.mode, "single");
        assert!(normalized.items.is_empty());
        assert_eq!(
            normalized.content.as_deref(),
            Some("Fallback newsletter body")
        );
        assert_eq!(
            normalized.summary.as_deref(),
            Some("Fallback newsletter body")
        );
    }

    #[test]
    fn normalize_llm_result_keeps_multi_mode_only_for_multiple_distinct_urls() {
        let normalized = normalize_llm_result(
            super::RawNewsletterLlmResult {
                mode: "single".to_string(),
                summary: None,
                content: None,
                items: Some(vec![
                    super::RawNewsletterItem {
                        title: Some("first".to_string()),
                        url: Some("https://example.com/1".to_string()),
                        summary: Some("summary one".to_string()),
                        content: None,
                    },
                    super::RawNewsletterItem {
                        title: Some("duplicate".to_string()),
                        url: Some("https://example.com/1".to_string()),
                        summary: Some("duplicate summary".to_string()),
                        content: None,
                    },
                    super::RawNewsletterItem {
                        title: Some("second".to_string()),
                        url: Some("https://example.com/2".to_string()),
                        summary: Some("summary two".to_string()),
                        content: None,
                    },
                ]),
            },
            "Fallback newsletter body",
        );

        assert_eq!(normalized.mode, "multi");
        assert_eq!(normalized.items.len(), 2);
        assert_eq!(normalized.items[0].url, "https://example.com/1");
        assert_eq!(normalized.items[1].url, "https://example.com/2");
    }

    #[test]
    fn normalize_llm_result_builds_concise_single_summary_when_missing() {
        let normalized = normalize_llm_result(
            super::RawNewsletterLlmResult {
                mode: "single".to_string(),
                summary: None,
                content: Some(format!(
                    "<p>{}</p>",
                    "A".repeat(NEWSLETTER_SINGLE_SUMMARY_MAX_CHARS + 20)
                )),
                items: None,
            },
            "Fallback newsletter body",
        );

        assert_eq!(normalized.mode, "single");
        let summary = normalized.summary.expect("missing fallback summary");
        assert!(summary.ends_with("..."));
        assert!(summary.len() > NEWSLETTER_SINGLE_SUMMARY_MAX_CHARS);
    }

    #[test]
    fn mock_messages_can_be_loaded_from_env_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("mock-imap.json");
        std::fs::write(
            &file_path,
            serde_json::to_string(&vec![
                "Subject: Test\\r\\n\\r\\nBody 1",
                "Subject: Test 2\\r\\n\\r\\nBody 2",
            ])
            .unwrap(),
        )
        .unwrap();

        unsafe {
            std::env::set_var("TESTING_MODE", "true");
            std::env::set_var(
                "HEADLESS_RSS_TEST_IMAP_MESSAGES_FILE",
                file_path.to_string_lossy().to_string(),
            );
        }

        let messages = load_mock_messages_from_env().unwrap().unwrap();
        assert_eq!(messages.len(), 2);
        assert!(
            String::from_utf8(messages[0].clone())
                .unwrap()
                .contains("Body 1")
        );

        unsafe {
            std::env::remove_var("HEADLESS_RSS_TEST_IMAP_MESSAGES_FILE");
            std::env::remove_var("TESTING_MODE");
        }
    }
}
