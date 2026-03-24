use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::{GlideConfig, OpenAiLlmConfig, PromptConfig};

use super::CleanupContext;

pub struct OpenAiLlmProvider {
    client: Client,
    config: OpenAiLlmConfig,
    prompt: PromptConfig,
    api_key: String,
}

impl OpenAiLlmProvider {
    pub fn new(config: GlideConfig) -> Result<Self> {
        let provider_config = config.llm.openai.clone();
        let api_key = provider_config.resolve_api_key()?;

        Ok(Self {
            client: Client::new(),
            config: provider_config,
            prompt: config.llm.prompt.clone(),
            api_key,
        })
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[async_trait::async_trait]
impl super::LlmProvider for OpenAiLlmProvider {
    async fn clean(&self, raw_text: &str, context: &CleanupContext) -> Result<String> {
        let user_prompt = build_user_prompt(raw_text, context);

        // Use the style's prompt if the target app matches, otherwise default
        let system_prompt = if let Some(target) = &context.target_app {
            self.prompt
                .styles
                .iter()
                .find(|s| s.apps.iter().any(|a| a.eq_ignore_ascii_case(target)))
                .map(|s| s.prompt.as_str())
                .unwrap_or(&self.prompt.system)
        } else {
            &self.prompt.system
        };

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
        };

        let response = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .context("failed to call OpenAI chat completions API")?
            .error_for_status()
            .context("OpenAI chat completions API returned an error status")?;

        let parsed: ChatCompletionResponse = response
            .json()
            .await
            .context("failed to parse OpenAI chat response")?;

        let cleaned = parsed
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .context("OpenAI chat response did not include any choices")?;

        Ok(cleaned)
    }

    fn name(&self) -> &'static str {
        "OpenAI GPT"
    }
}

fn build_user_prompt(raw_text: &str, context: &CleanupContext) -> String {
    let mut prompt = String::new();

    if let Some(target_app) = &context.target_app {
        prompt.push_str(&format!("Target app: {target_app}\n"));
    }
    if let Some(mode_hint) = &context.mode_hint {
        prompt.push_str(&format!("Writing mode: {mode_hint}\n"));
    }

    prompt.push_str("Transcript:\n");
    prompt.push_str(raw_text);
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_user_prompt_minimal() {
        let ctx = CleanupContext::default();
        let prompt = build_user_prompt("hello world", &ctx);
        assert_eq!(prompt, "Transcript:\nhello world");
    }

    #[test]
    fn test_build_user_prompt_with_target_app() {
        let ctx = CleanupContext {
            target_app: Some("Slack".to_string()),
            mode_hint: None,
        };
        let prompt = build_user_prompt("test", &ctx);
        assert!(prompt.contains("Target app: Slack\n"));
        assert!(prompt.contains("Transcript:\ntest"));
    }

    #[test]
    fn test_build_user_prompt_with_mode_hint() {
        let ctx = CleanupContext {
            target_app: None,
            mode_hint: Some("email".to_string()),
        };
        let prompt = build_user_prompt("test", &ctx);
        assert!(prompt.contains("Writing mode: email\n"));
        assert!(prompt.contains("Transcript:\ntest"));
    }

    #[test]
    fn test_build_user_prompt_full() {
        let ctx = CleanupContext {
            target_app: Some("VSCode".to_string()),
            mode_hint: Some("code comment".to_string()),
        };
        let prompt = build_user_prompt("fix the bug", &ctx);
        assert!(prompt.starts_with("Target app: VSCode\n"));
        assert!(prompt.contains("Writing mode: code comment\n"));
        assert!(prompt.ends_with("Transcript:\nfix the bug"));
    }
}
