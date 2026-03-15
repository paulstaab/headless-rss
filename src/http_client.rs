use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};

const FEED_CONNECT_TIMEOUT_SECONDS: u64 = 5;
const FEED_READ_TIMEOUT_SECONDS: u64 = 15;
const FEED_REQUEST_TIMEOUT_SECONDS: u64 = 20;

/// Builds a reqwest client for article fetches with browser-like headers.
pub fn build_article_http_client() -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        HeaderValue::from_static("en-US,en;q=0.9"),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .user_agent(
            "Mozilla/5.0 (compatible; headless-rss/1.0; +https://github.com/paulstaab/headless-rss)",
        )
        .connect_timeout(Duration::from_secs(FEED_CONNECT_TIMEOUT_SECONDS))
        .read_timeout(Duration::from_secs(FEED_READ_TIMEOUT_SECONDS))
        .timeout(Duration::from_secs(FEED_REQUEST_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("failed to build article http client")
}

/// Builds a reqwest client for feed fetches with explicit headers and timeouts.
///
/// A shared client is intended to be reused across requests to avoid rebuilding
/// connection pools and to prevent indefinite hangs against slow origins.
pub fn build_feed_http_client() -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        HeaderValue::from_static(
            "application/rss+xml, application/atom+xml, application/xml, text/xml;q=0.9, */*;q=0.8",
        ),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        HeaderValue::from_static("en-US,en;q=0.9"),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .user_agent(
            "Mozilla/5.0 (compatible; headless-rss/1.0; +https://github.com/paulstaab/headless-rss)",
        )
        .connect_timeout(Duration::from_secs(FEED_CONNECT_TIMEOUT_SECONDS))
        .read_timeout(Duration::from_secs(FEED_READ_TIMEOUT_SECONDS))
        .timeout(Duration::from_secs(FEED_REQUEST_TIMEOUT_SECONDS))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .context("failed to build feed http client")
}
