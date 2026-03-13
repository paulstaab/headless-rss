//! Shared helpers for OpenAI-compatible chat completion requests.

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::config::Config;

const OPENAI_TIMEOUT_SECONDS: u64 = 10;

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

/// Sends a structured chat-completion request and returns the first non-empty message content.
pub async fn request_chat_completion_content(
    config: &Config,
    payload: serde_json::Value,
    operation: &str,
) -> Option<String> {
    let api_key = config.openai_api_key.as_deref()?;

    let client = match Client::builder()
        .timeout(Duration::from_secs(OPENAI_TIMEOUT_SECONDS))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!(error = %err, operation, "failed to build OpenAI client");
            return None;
        }
    };

    let response = match client
        .post(openai_chat_completions_url(&config.openai_base_url))
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!(error = %err, operation, "OpenAI request failed");
            return None;
        }
    };

    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), operation, "OpenAI request returned non-success status");
        return None;
    }

    let body: OpenAiChatCompletionResponse = match response.json().await {
        Ok(body) => body,
        Err(err) => {
            tracing::warn!(error = %err, operation, "failed to decode OpenAI response");
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
