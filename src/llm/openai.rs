use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    benchmark::ProfileCollector,
    config::{Provider, ProvidersConfig},
};

use super::{CleanupContext, build_cleanup_user_prompt};

pub struct OpenAiLlmProvider {
    client: Client,
    provider: Provider,
    endpoint: String,
    model: String,
    system_prompt: String,
    api_key: String,
    profile: ProfileCollector,
}

impl OpenAiLlmProvider {
    pub fn new(
        provider: Provider,
        model: &str,
        system_prompt: &str,
        providers: &ProvidersConfig,
        profile: ProfileCollector,
    ) -> Result<Self> {
        let creds = providers.credentials_for(provider);
        let api_key = creds.resolve_api_key("LLM")?;
        let endpoint = provider.llm_endpoint(&creds.base_url);
        Ok(Self {
            client: Client::new(),
            provider,
            endpoint,
            model: model.to_string(),
            system_prompt: system_prompt.to_string(),
            api_key,
            profile,
        })
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
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
        let total_started = std::time::Instant::now();
        let request_started = std::time::Instant::now();
        let user_prompt = build_cleanup_user_prompt(raw_text, context);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            temperature: deterministic_temperature(self.provider, &self.model),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: self.system_prompt.clone(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt,
                },
            ],
        };
        self.profile
            .record("remote_llm_json_request_build", request_started.elapsed());

        let send_started = std::time::Instant::now();
        self.profile
            .record_since_marker("llm_start", "llm_start_to_llm_http_send_start");
        self.profile
            .record_since_marker("flow_release", "flow_release_to_llm_http_send_start");
        self.profile
            .record_since_marker("flow_stt_result", "flow_stt_result_to_llm_http_send_start");
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to call {} chat completions API",
                    self.provider.label()
                )
            })?;
        let status = response.status();
        self.profile
            .record("remote_llm_http_send_status", send_started.elapsed());
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|error| format!("<failed to read error response body: {error}>"));
            anyhow::bail!(
                "{} chat completions API returned HTTP {status}: {}",
                self.provider.label(),
                capped_error_body(&body)
            );
        }

        let parse_started = std::time::Instant::now();
        let parsed: ChatCompletionResponse = response
            .json()
            .await
            .context("failed to parse OpenAI chat response")?;
        self.profile
            .record("remote_llm_response_parse", parse_started.elapsed());

        let cleaned = parsed
            .choices
            .first()
            .map(|choice| choice.message.content.trim().to_string())
            .context("OpenAI chat response did not include any choices")?;
        self.profile
            .record("remote_llm_provider_total", total_started.elapsed());

        Ok(cleaned)
    }

    fn name(&self) -> &'static str {
        "OpenAI GPT"
    }
}

const ERROR_BODY_CHAR_LIMIT: usize = 4096;

fn deterministic_temperature(provider: Provider, model: &str) -> Option<f32> {
    if provider == Provider::OpenAi && openai_model_rejects_temperature_zero(model) {
        None
    } else {
        Some(0.0)
    }
}

fn openai_model_rejects_temperature_zero(model: &str) -> bool {
    let model = model.trim().to_lowercase();
    model.starts_with("gpt-5") || model.starts_with('o')
}

fn capped_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "<empty response body>".to_string();
    }
    if trimmed.chars().count() <= ERROR_BODY_CHAR_LIMIT {
        return trimmed.to_string();
    }

    let prefix = trimmed
        .chars()
        .take(ERROR_BODY_CHAR_LIMIT)
        .collect::<String>();
    format!("{prefix}... [truncated]")
}

#[cfg(test)]
mod tests;
