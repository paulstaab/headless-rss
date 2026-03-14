//! Shared helpers for OpenAI-compatible chat completion requests.

use std::error::Error as StdError;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::config::Config;

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

/// Builds the chat completions URL from the configured OpenAI-compatible base URL.
pub fn openai_chat_completions_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

/// Formats an error with its full source chain for actionable warning logs.
fn format_error_chain(error: &dyn StdError) -> String {
    let mut chain = vec![error.to_string()];
    let mut source = error.source();

    while let Some(next) = source {
        chain.push(next.to_string());
        source = next.source();
    }

    chain.join(": ")
}

/// Sends a structured chat-completion request and returns the first non-empty message content.
pub async fn request_chat_completion_content(
    config: &Config,
    payload: serde_json::Value,
    operation: &str,
) -> Option<String> {
    let api_key = config.openai_api_key.as_deref()?;

    let client = match Client::builder()
        .timeout(Duration::from_secs(config.openai_timeout_seconds))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!(
                error = %format_error_chain(&err),
                operation,
                "failed to build OpenAI client"
            );
            return None;
        }
    };

    let request_url = openai_chat_completions_url(&config.openai_base_url);

    let response = match client
        .post(&request_url)
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            tracing::error!(
                is_timeout = err.is_timeout(),
                is_connect = err.is_connect(),
                is_request = err.is_request(),
                timeout_seconds = config.openai_timeout_seconds,
                operation,
                "OpenAI request failed"
            );
            return None;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        if let Err(err) = response.text().await {
            let _ = err;
            tracing::warn!(
                status = %status,
                operation,
                "failed to read OpenAI error response body"
            );
        }

        tracing::warn!(
            status = %status,
            operation,
            "OpenAI request returned non-success status"
        );
        return None;
    }

    let response_body = match response.text().await {
        Ok(body) => body,
        Err(err) => {
            let _ = err;
            tracing::warn!(operation, "failed to read OpenAI response body");
            return None;
        }
    };

    let body: OpenAiChatCompletionResponse = match serde_json::from_str(&response_body) {
        Ok(body) => body,
        Err(err) => {
            let _ = err;
            tracing::warn!(operation, "failed to decode OpenAI response");
            return None;
        }
    };

    body.choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;
    use std::fmt;

    use super::{format_error_chain, openai_chat_completions_url};

    #[derive(Debug)]
    struct TestError {
        message: &'static str,
        source: Option<Box<dyn StdError + Send + Sync>>,
    }

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl StdError for TestError {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            self.source
                .as_deref()
                .map(|source| source as &(dyn StdError + 'static))
        }
    }

    #[test]
    fn openai_chat_url_respects_custom_base_url() {
        assert_eq!(
            openai_chat_completions_url("http://127.0.0.1:3000/v1/"),
            "http://127.0.0.1:3000/v1/chat/completions"
        );
    }

    #[test]
    fn format_error_chain_includes_nested_sources() {
        let error = TestError {
            message: "top-level",
            source: Some(Box::new(TestError {
                message: "mid-level",
                source: Some(Box::new(TestError {
                    message: "root-cause",
                    source: None,
                })),
            })),
        };

        assert_eq!(
            format_error_chain(&error),
            "top-level: mid-level: root-cause"
        );
    }
}
