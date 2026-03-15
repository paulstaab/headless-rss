//! Shared content extraction and summarization helpers for Rust article ingestion.

use std::sync::OnceLock;

use anyhow::{Context, Result};
use feed_rs::model::Entry;
use readability_js::Readability;
use regex::Regex;
use reqwest::Client;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::config::Config;
use crate::llm::{self, LlmRequestContext};
use crate::ssrf;

const ONE_DAY: i64 = 86_400;
const ONE_MONTH: i64 = 30 * ONE_DAY;
const ARTICLE_MAX_CHARS: usize = 8_000;
const LLM_SUMMARY_MIN_CHARS: usize = 160;
const ARTICLE_SUMMARY_SCHEMA_NAME: &str = "article_summary";
const SUMMARY_QUALITY_SCHEMA_NAME: &str = "summary_quality";

#[derive(Clone, Copy, Debug)]
pub struct FeedContentState {
    pub last_quality_check: Option<i64>,
    pub use_extracted_fulltext: bool,
    pub use_llm_summary: bool,
}

#[derive(Debug)]
pub struct EnrichedArticleContent {
    pub content: Option<String>,
    pub summary: Option<String>,
    pub media_thumbnail: Option<String>,
    pub content_hash: Option<String>,
}

/// Extracts the first image source URL from HTML body content.
pub fn extract_first_image_url(html_content: Option<&str>) -> Option<String> {
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

/// Normalizes HTML or plain text for length-based quality comparisons.
pub fn normalize_text(text: Option<&str>) -> String {
    let Some(text) = text else {
        return String::new();
    };

    static HTML_REGEX: OnceLock<Regex> = OnceLock::new();
    static WHITESPACE_REGEX: OnceLock<Regex> = OnceLock::new();

    let stripped = HTML_REGEX
        .get_or_init(|| Regex::new(r"<[^>]+>").expect("valid html strip regex"))
        .replace_all(text, " ");
    let collapsed = WHITESPACE_REGEX
        .get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"))
        .replace_all(&stripped, " ");

    collapsed.trim().to_lowercase()
}

/// Extracts clean article HTML from a fetched document using Mozilla Readability.
pub fn extract_article_from_html(html: &str, base_url: Option<&str>) -> Option<String> {
    if html.trim().is_empty() {
        return None;
    }

    let reader = Readability::new().ok()?;
    let article = match base_url {
        Some(url) => reader
            .parse_with_url(html, url)
            .or_else(|_| reader.parse(html))
            .ok()?,
        None => reader.parse(html).ok()?,
    };

    let content = article.content.trim();
    if content.is_empty() {
        return None;
    }

    Some(content.to_string())
}

/// Refreshes feed quality flags when the monthly evaluation window has elapsed.
pub async fn maybe_refresh_feed_content_state(
    pool: &SqlitePool,
    article_http_client: &Client,
    config: &Config,
    feed_id: i64,
    current_state: FeedContentState,
    entries: &[Entry],
) -> Result<FeedContentState> {
    if !needs_quality_check(current_state.last_quality_check) {
        return Ok(current_state);
    }

    tracing::info!(feed_id, "performing feed content quality check");
    let Some(sample) = select_quality_sample(entries) else {
        tracing::info!(
            feed_id,
            "no suitable article found for content quality check"
        );
        return Ok(current_state);
    };

    let feed_content = sample
        .content
        .as_ref()
        .and_then(|content| content.body.as_deref())
        .or_else(|| {
            sample
                .summary
                .as_ref()
                .map(|summary| summary.content.as_str())
        });
    let feed_summary = sample
        .summary
        .as_ref()
        .map(|summary| summary.content.as_str());
    let article_url = sample.links.first().map(|link| link.href.as_str());
    let extracted_text = match article_url {
        Some(url) => extract_article(article_http_client, config, Some(feed_id), None, url).await,
        None => None,
    };
    let use_extracted_fulltext =
        is_extracted_content_preferred(extracted_text.as_deref(), feed_content);
    let final_article_text = if use_extracted_fulltext {
        extracted_text.as_deref()
    } else {
        feed_content
    };
    let use_llm_summary =
        should_enable_llm_summary(config, feed_id, final_article_text, feed_summary).await;
    let next_state = FeedContentState {
        last_quality_check: Some(unix_now()),
        use_extracted_fulltext,
        use_llm_summary,
    };

    sqlx::query(
        "UPDATE feed SET last_quality_check = ?, use_extracted_fulltext = ?, use_llm_summary = ? WHERE id = ?",
    )
    .bind(next_state.last_quality_check)
    .bind(next_state.use_extracted_fulltext)
    .bind(next_state.use_llm_summary)
    .bind(feed_id)
    .execute(pool)
    .await
    .context("failed to persist feed content quality flags")?;

    tracing::info!(
        feed_id,
        use_extracted_fulltext = next_state.use_extracted_fulltext,
        use_llm_summary = next_state.use_llm_summary,
        "feed content quality check completed"
    );

    Ok(next_state)
}

/// Applies thumbnail fallback, optional full-text extraction, and optional summary generation.
#[allow(clippy::too_many_arguments)]
pub async fn enrich_article_content(
    article_http_client: &Client,
    config: &Config,
    feed_id: Option<i64>,
    article_id: Option<i64>,
    url: Option<&str>,
    content: Option<String>,
    summary: Option<String>,
    media_thumbnail: Option<String>,
    use_extracted_fulltext: bool,
    use_llm_summary: bool,
) -> EnrichedArticleContent {
    let mut final_content = content;
    let mut final_summary = summary;
    let mut final_media_thumbnail = media_thumbnail
        .or_else(|| extract_first_image_url(final_content.as_deref().or(final_summary.as_deref())));

    if use_extracted_fulltext
        && let Some(article_url) = url
        && let Some(extracted_content) = extract_article(
            article_http_client,
            config,
            feed_id,
            article_id,
            article_url,
        )
        .await
    {
        final_content = Some(extracted_content);
        if final_media_thumbnail.is_none() {
            final_media_thumbnail =
                extract_first_image_url(final_content.as_deref().or(final_summary.as_deref()));
        }
    }

    if use_llm_summary {
        final_summary = None;
    }

    if final_summary.is_none()
        && let Some(content_text) = final_content.as_deref()
    {
        let llm_summary = if use_llm_summary
            && config.llm_enabled()
            && content_text.chars().count() >= LLM_SUMMARY_MIN_CHARS
        {
            summarize_article_with_llm(config, feed_id, article_id, content_text).await
        } else {
            None
        };

        final_summary = build_missing_summary(
            content_text,
            use_llm_summary,
            config.llm_enabled(),
            llm_summary,
        );
    }

    let content_hash = final_content
        .as_ref()
        .map(|value| format!("{:x}", md5::compute(value.as_bytes())));

    EnrichedArticleContent {
        content: final_content,
        summary: final_summary,
        media_thumbnail: final_media_thumbnail,
        content_hash,
    }
}

/// Returns whether the monthly feed-quality evaluation window has elapsed.
fn needs_quality_check(last_quality_check: Option<i64>) -> bool {
    match last_quality_check {
        None => true,
        Some(last_quality_check) => unix_now() - last_quality_check > ONE_MONTH,
    }
}

/// Picks the first feed entry that includes both content metadata and a canonical link.
fn select_quality_sample(entries: &[Entry]) -> Option<&Entry> {
    entries.iter().find(|entry| {
        let has_link = !entry.links.is_empty();
        let has_content = entry
            .content
            .as_ref()
            .and_then(|content| content.body.as_ref())
            .is_some()
            || entry.summary.is_some();

        has_link && has_content
    })
}

/// Compares extracted article text against feed-provided text to decide which is richer.
fn is_extracted_content_preferred(
    extracted_text: Option<&str>,
    feed_content: Option<&str>,
) -> bool {
    let extracted_length = normalize_text(extracted_text).chars().count();
    if extracted_length == 0 {
        return false;
    }

    let feed_length = normalize_text(feed_content).chars().count();
    extracted_length >= 2 * feed_length
}

/// Builds a fallback summary when feed content lacks a dedicated summary field.
fn build_missing_summary(
    content: &str,
    use_llm_summary: bool,
    llm_enabled: bool,
    llm_summary: Option<String>,
) -> Option<String> {
    if content.chars().count() < LLM_SUMMARY_MIN_CHARS {
        return Some(content.to_string());
    }

    if use_llm_summary
        && llm_enabled
        && let Some(summary) = llm_summary
    {
        return Some(summary);
    }

    Some(format!(
        "{}...",
        truncate_chars(content, LLM_SUMMARY_MIN_CHARS)
    ))
}

async fn should_enable_llm_summary(
    config: &Config,
    feed_id: i64,
    final_article_text: Option<&str>,
    feed_summary: Option<&str>,
) -> bool {
    let normalized_summary = normalize_text(feed_summary);
    if normalized_summary.is_empty() {
        return true;
    }

    let normalized_article_text = normalize_text(final_article_text);
    if normalized_article_text.is_empty() {
        return false;
    }

    if config.llm_enabled() {
        let article_text = plain_text(final_article_text);
        let summary_text = plain_text(feed_summary);
        if let Some(is_good) =
            is_good_standalone_summary_with_llm(config, feed_id, &article_text, &summary_text).await
        {
            return !is_good;
        }
    }

    should_enable_llm_summary_by_heuristic(feed_summary, final_article_text)
}

fn should_enable_llm_summary_by_heuristic(
    feed_summary: Option<&str>,
    final_article_text: Option<&str>,
) -> bool {
    let normalized_summary = normalize_text(feed_summary);
    if normalized_summary.is_empty() {
        return true;
    }

    let normalized_article_text = normalize_text(final_article_text);
    if normalized_article_text.is_empty() {
        return false;
    }

    normalized_article_text.chars().count() > normalized_summary.chars().count()
        && normalized_article_text.starts_with(&normalized_summary)
}

/// Fetches a remote article document, logs the extraction request, and extracts cleaned main-content HTML.
async fn extract_article(
    article_http_client: &Client,
    config: &Config,
    feed_id: Option<i64>,
    article_id: Option<i64>,
    url: &str,
) -> Option<String> {
    tracing::info!(
        feed_id,
        article_id,
        loaded_url = url,
        "starting article extraction"
    );

    let response =
        match ssrf::get_with_safe_redirects(article_http_client, url, config.testing_mode).await {
            Ok(response) => response,
            Err(ssrf::SafeGetError::Validation(err)) => {
                tracing::warn!(
                    feed_id,
                    article_id,
                    loaded_url = url,
                    error = %err,
                    "blocked article url for extraction"
                );
                return None;
            }
            Err(ssrf::SafeGetError::Request(err)) => {
                tracing::warn!(
                    feed_id,
                    article_id,
                    loaded_url = url,
                    error = %err,
                    "failed to fetch article url for extraction"
                );
                return None;
            }
        };

    if !response.status().is_success() {
        tracing::warn!(
            feed_id,
            article_id,
            loaded_url = url,
            status = %response.status(),
            "article extraction fetch returned non-success status"
        );
        return None;
    }

    let html = match response.text().await {
        Ok(html) => html,
        Err(err) => {
            tracing::warn!(
                feed_id,
                article_id,
                loaded_url = url,
                error = %err,
                "failed to read article response body"
            );
            return None;
        }
    };

    let extracted = extract_article_from_html(&html, Some(url));
    if extracted.is_none() {
        tracing::warn!(
            feed_id,
            article_id,
            loaded_url = url,
            "article extraction produced empty content"
        );
    }
    extracted
}

/// Requests a structured summary for extracted article content when LLM support is enabled.
async fn summarize_article_with_llm(
    config: &Config,
    feed_id: Option<i64>,
    article_id: Option<i64>,
    article_text: &str,
) -> Option<String> {
    config.openai_api_key.as_deref()?;

    let plain_text = plain_text(Some(article_text));
    let trimmed_text = truncate_chars(&plain_text, ARTICLE_MAX_CHARS);
    if trimmed_text.trim().is_empty() {
        return None;
    }

    let response_text = llm::request_chat_completion_content(
        config,
        build_openai_summary_payload(&config.openai_model, &trimmed_text),
        LlmRequestContext {
            task_name: "article-summarization",
            feed_id,
            article_id,
        },
    )
    .await?;

    let parsed = parse_llm_json_response(&response_text, "llm summary")?;
    let summary = extract_string_from_structured_response(
        &parsed,
        "summary",
        &[ARTICLE_SUMMARY_SCHEMA_NAME],
    )?;

    Some(format!("{summary} (AI generated)"))
}

async fn is_good_standalone_summary_with_llm(
    config: &Config,
    feed_id: i64,
    article_text: &str,
    summary: &str,
) -> Option<bool> {
    config.openai_api_key.as_deref()?;

    let article_text = plain_text(Some(article_text));
    let summary = plain_text(Some(summary));
    if article_text.trim().is_empty() || summary.trim().is_empty() {
        return None;
    }

    let response_text = llm::request_chat_completion_content(
        config,
        build_openai_summary_quality_payload(&config.openai_model, &article_text, &summary),
        LlmRequestContext {
            task_name: "summary-quality-evaluation",
            feed_id: Some(feed_id),
            article_id: None,
        },
    )
    .await?;

    let parsed = parse_llm_json_response(&response_text, "summary quality")?;

    extract_bool_from_structured_response(&parsed, "is_good", &[SUMMARY_QUALITY_SCHEMA_NAME])
}

fn parse_llm_json_response(response_text: &str, response_kind: &str) -> Option<Value> {
    match serde_json::from_str(response_text) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            tracing::warn!(error = %err, response_kind, "structured LLM response was not valid JSON");
            None
        }
    }
}

fn extract_string_from_structured_response(
    parsed: &Value,
    field_name: &str,
    wrapper_keys: &[&str],
) -> Option<String> {
    if let Some(value) = parsed
        .get(field_name)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(value.to_string());
    }

    for wrapper_key in wrapper_keys {
        if let Some(value) = parsed
            .get(wrapper_key)
            .and_then(|value| value.get(field_name))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            tracing::warn!(
                wrapper_key,
                field_name,
                "unwrapped structured LLM response field"
            );
            return Some(value.to_string());
        }
    }

    None
}

fn extract_bool_from_structured_response(
    parsed: &Value,
    field_name: &str,
    wrapper_keys: &[&str],
) -> Option<bool> {
    if let Some(value) = parsed.get(field_name).and_then(|value| value.as_bool()) {
        return Some(value);
    }

    for wrapper_key in wrapper_keys {
        if let Some(value) = parsed
            .get(wrapper_key)
            .and_then(|value| value.get(field_name))
            .and_then(|value| value.as_bool())
        {
            tracing::warn!(
                wrapper_key,
                field_name,
                "unwrapped structured LLM response field"
            );
            return Some(value);
        }
    }

    None
}

/// Builds the structured-output payload used for article summarization requests.
fn build_openai_summary_payload(model: &str, article_text: &str) -> serde_json::Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "Summarize the article clearly and concisely. Return 3-6 sentences, no bullets, no headings, plain text only. Return exactly one JSON object with a top-level `summary` field and no wrapper object."
            },
            {
                "role": "user",
                "content": format!("Article:\n{article_text}")
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": ARTICLE_SUMMARY_SCHEMA_NAME,
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "summary": {"type": "string"}
                    },
                    "required": ["summary"],
                    "additionalProperties": false
                }
            }
        }
    })
}

fn build_openai_summary_quality_payload(
    model: &str,
    article_text: &str,
    summary: &str,
) -> serde_json::Value {
    json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are evaluating whether a summary is a good standalone summary of an article. Return exactly one JSON object with a top-level `is_good` boolean field and no wrapper object. Set is_good=true if it captures the main points and is not just a lead-in."
            },
            {
                "role": "user",
                "content": format!("Article:\n{article_text}\n\nSummary:\n{summary}")
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": SUMMARY_QUALITY_SCHEMA_NAME,
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "is_good": {"type": "boolean"}
                    },
                    "required": ["is_good"],
                    "additionalProperties": false
                }
            }
        }
    })
}

fn plain_text(text: Option<&str>) -> String {
    let Some(text) = text else {
        return String::new();
    };

    static WHITESPACE_REGEX: OnceLock<Regex> = OnceLock::new();

    let stripped = strip_html(text);
    WHITESPACE_REGEX
        .get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"))
        .replace_all(&stripped, " ")
        .trim()
        .to_string()
}

/// Removes HTML tags so LLM prompts operate on readable text instead of markup.
fn strip_html(text: &str) -> String {
    static HTML_REGEX: OnceLock<Regex> = OnceLock::new();
    HTML_REGEX
        .get_or_init(|| Regex::new(r"<[^>]+>").expect("valid html strip regex"))
        .replace_all(text, " ")
        .to_string()
}

/// Truncates a string by Unicode scalar count without breaking UTF-8 boundaries.
fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// Returns the current Unix timestamp in seconds for feed quality bookkeeping.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        ARTICLE_SUMMARY_SCHEMA_NAME, SUMMARY_QUALITY_SCHEMA_NAME, build_missing_summary,
        extract_article_from_html, extract_bool_from_structured_response, extract_first_image_url,
        extract_string_from_structured_response, is_extracted_content_preferred, normalize_text,
        parse_llm_json_response, plain_text, should_enable_llm_summary_by_heuristic,
    };
    use crate::llm::openai_chat_completions_url;

    #[test]
    fn extract_article_from_html_returns_main_article_body() {
        let html = r#"
<!DOCTYPE html>
<html>
  <body>
    <nav>Navigation links</nav>
    <article>
      <h1>Example article</h1>
      <p>Main body paragraph one.</p>
      <p>Main body paragraph two.</p>
    </article>
    <footer>Footer text that should be excluded.</footer>
  </body>
</html>
"#;

        let extracted = extract_article_from_html(html, Some("https://example.com/article"))
            .expect("expected extracted article html");

        assert!(extracted.contains("Main body paragraph one."));
        assert!(extracted.contains("Main body paragraph two."));
        assert!(!extracted.contains("Footer text that should be excluded."));
    }

    #[test]
    fn normalize_text_removes_html_and_collapses_whitespace() {
        let normalized = normalize_text(Some("<p>Hello</p>\n\t <strong>World</strong>"));
        assert_eq!(normalized, "hello world");
    }

    #[test]
    fn fulltext_quality_prefers_significantly_longer_content() {
        assert!(is_extracted_content_preferred(
            Some("This extracted article text is much longer than the teaser summary."),
            Some("Short teaser.")
        ));
    }

    #[test]
    fn fulltext_quality_rejects_short_extractions() {
        assert!(!is_extracted_content_preferred(
            Some("Short."),
            Some("This feed summary already contains a reasonable amount of detail.")
        ));
    }

    #[test]
    fn summary_generation_truncates_when_llm_is_disabled() {
        let content = "a".repeat(200);
        let summary =
            build_missing_summary(&content, false, false, None).expect("expected summary");
        assert_eq!(summary, format!("{}...", "a".repeat(160)));
    }

    #[test]
    fn summary_generation_keeps_llm_suffix() {
        let content = "a".repeat(200);
        let summary = build_missing_summary(
            &content,
            true,
            true,
            Some("Generated summary (AI generated)".to_string()),
        )
        .expect("expected llm summary");

        assert_eq!(summary, "Generated summary (AI generated)");
    }

    #[test]
    fn summary_generation_falls_back_to_truncation_when_llm_returns_none() {
        let content = "a".repeat(200);
        let summary = build_missing_summary(&content, true, true, None).expect("expected summary");
        assert_eq!(summary, format!("{}...", "a".repeat(160)));
    }

    #[test]
    fn structured_summary_response_accepts_top_level_summary() {
        let parsed = parse_llm_json_response(r#"{"summary":"Top-level summary."}"#, "llm summary")
            .expect("expected parsed response");

        assert_eq!(
            extract_string_from_structured_response(
                &parsed,
                "summary",
                &[ARTICLE_SUMMARY_SCHEMA_NAME]
            )
            .as_deref(),
            Some("Top-level summary.")
        );
    }

    #[test]
    fn structured_summary_response_accepts_schema_wrapped_summary() {
        let parsed = parse_llm_json_response(
            r#"{"article_summary":{"summary":"Wrapped summary."}}"#,
            "llm summary",
        )
        .expect("expected parsed response");

        assert_eq!(
            extract_string_from_structured_response(
                &parsed,
                "summary",
                &[ARTICLE_SUMMARY_SCHEMA_NAME]
            )
            .as_deref(),
            Some("Wrapped summary.")
        );
    }

    #[test]
    fn structured_summary_quality_response_accepts_schema_wrapper() {
        let parsed =
            parse_llm_json_response(r#"{"summary_quality":{"is_good":true}}"#, "summary quality")
                .expect("expected parsed response");

        assert_eq!(
            extract_bool_from_structured_response(
                &parsed,
                "is_good",
                &[SUMMARY_QUALITY_SCHEMA_NAME]
            ),
            Some(true)
        );
    }

    #[test]
    fn summary_quality_heuristic_enables_llm_when_summary_is_article_prefix() {
        assert!(should_enable_llm_summary_by_heuristic(
            Some("Lead sentence only."),
            Some("Lead sentence only. More detail follows in the full article body.")
        ));
    }

    #[test]
    fn summary_quality_heuristic_disables_llm_when_summary_is_not_prefix() {
        assert!(!should_enable_llm_summary_by_heuristic(
            Some("Different summary."),
            Some("Lead sentence only. More detail follows in the full article body.")
        ));
    }

    #[test]
    fn summary_quality_heuristic_enables_llm_when_summary_is_missing() {
        assert!(should_enable_llm_summary_by_heuristic(
            None,
            Some("Lead sentence only. More detail follows in the full article body.")
        ));
    }

    #[test]
    fn plain_text_strips_html_and_preserves_word_case() {
        let result = plain_text(Some("<p>Hello</p>\n<strong>World</strong>"));
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn extract_first_image_url_finds_first_image() {
        let html = "<p>Lead</p><img src='https://example.com/1.jpg' /><img src='https://example.com/2.jpg' />";
        assert_eq!(
            extract_first_image_url(Some(html)).as_deref(),
            Some("https://example.com/1.jpg")
        );
    }

    #[test]
    fn openai_chat_url_respects_custom_base_url() {
        assert_eq!(
            openai_chat_completions_url("http://127.0.0.1:3000/v1/"),
            "http://127.0.0.1:3000/v1/chat/completions"
        );
    }
}
