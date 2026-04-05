use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CleanupConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub system_prompt: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

pub async fn cleanup(raw_text: &str, config: &CleanupConfig) -> Result<String> {
    let _ = &config.provider;
    let endpoint = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let request = ChatCompletionRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: config.system_prompt.clone(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: raw_text.to_string(),
            },
        ],
    };

    let response = Client::new()
        .post(&endpoint)
        .bearer_auth(&config.api_key)
        .json(&request)
        .send()
        .await
        .context("failed to call chat completions API")?
        .error_for_status()
        .context("chat completions API returned an error status")?;

    let parsed: ChatCompletionResponse = response
        .json()
        .await
        .context("failed to parse chat response")?;
    let text = parsed
        .choices
        .first()
        .map(|choice| choice.message.content.trim().to_string())
        .unwrap_or_default();

    Ok(strip_think_tags(&text))
}

fn strip_think_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.to_lowercase().find("<think") {
        result.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].to_lowercase().find("</think") {
            let close_end = remaining[start + end..]
                .find('>')
                .map(|index| start + end + index + 1)
                .unwrap_or(remaining.len());
            remaining = &remaining[close_end..];
        } else {
            remaining = "";
        }
    }
    result.push_str(remaining);
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::strip_think_tags;

    #[test]
    fn removes_reasoning_blocks() {
        assert_eq!(strip_think_tags("<think>reasoning</think>Hello"), "Hello");
        assert_eq!(
            strip_think_tags("Hi <think>reasoning</think>there"),
            "Hi there"
        );
    }

    #[test]
    fn removes_unclosed_reasoning_block() {
        assert_eq!(strip_think_tags("Answer<think>hidden"), "Answer");
    }
}
