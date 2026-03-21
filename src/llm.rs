//! Shared helpers for OpenAI-compatible chat completion requests.

use std::error::Error as StdError;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::config::Config;

/// Structured metadata describing one LLM task invocation.
#[derive(Clone, Copy, Debug)]
pub struct LlmRequestContext {
    pub task_name: &'static str,
    pub feed_id: Option<i64>,
    pub article_id: Option<i64>,
}

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
    context: LlmRequestContext,
) -> Option<String> {
    let api_key = config.openai_api_key.as_deref()?;

    let client = match Client::builder()
        .timeout(Duration::from_secs(config.openai_timeout_seconds))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::error!(
                error = %format_error_chain(&err),
                task_name = context.task_name,
                feed_id = context.feed_id,
                article_id = context.article_id,
                "failed to build OpenAI client"
            );
            return None;
        }
    };

    let request_url = openai_chat_completions_url(&config.openai_base_url);

    tracing::info!(
        task_name = context.task_name,
        feed_id = context.feed_id,
        article_id = context.article_id,
        timeout_seconds = config.openai_timeout_seconds,
        "starting LLM request"
    );

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
                task_name = context.task_name,
                feed_id = context.feed_id,
                article_id = context.article_id,
                "OpenAI request failed"
            );
            return None;
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        match response.text().await {
            Ok(body) => tracing::error!(
                status = %status,
                response_body = %body,
                task_name = context.task_name,
                feed_id = context.feed_id,
                article_id = context.article_id,
                "OpenAI request returned non-success status"
            ),
            Err(err) => tracing::error!(
                status = %status,
                error = %err,
                task_name = context.task_name,
                feed_id = context.feed_id,
                article_id = context.article_id,
                "OpenAI request returned non-success status; failed to read response body"
            ),
        }
        return None;
    }

    let response_body = match response.text().await {
        Ok(body) => body,
        Err(err) => {
            tracing::error!(
                error = %err,
                task_name = context.task_name,
                feed_id = context.feed_id,
                article_id = context.article_id,
                "failed to read OpenAI response body"
            );
            return None;
        }
    };

    let body: OpenAiChatCompletionResponse = match serde_json::from_str(&response_body) {
        Ok(body) => body,
        Err(err) => {
            tracing::error!(
                error = %err,
                task_name = context.task_name,
                feed_id = context.feed_id,
                article_id = context.article_id,
                "failed to decode OpenAI response"
            );
            return None;
        }
    };

    tracing::info!(
        task_name = context.task_name,
        feed_id = context.feed_id,
        article_id = context.article_id,
        "LLM request completed"
    );

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

    use super::{LlmRequestContext, format_error_chain, openai_chat_completions_url};

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

    #[test]
    fn llm_request_context_carries_identifiers() {
        let context = LlmRequestContext {
            task_name: "article-summarization",
            feed_id: Some(12),
            article_id: Some(34),
        };

        assert_eq!(context.task_name, "article-summarization");
        assert_eq!(context.feed_id, Some(12));
        assert_eq!(context.article_id, Some(34));
    }
}
