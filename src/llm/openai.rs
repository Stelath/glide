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
mod tests {
    use super::*;
    use crate::{
        benchmark::ProfileCollector,
        config::{Provider, providers::ProviderCredentials},
        llm::LlmProvider,
    };
    use serde_json::Value;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[test]
    fn test_build_user_prompt_minimal() {
        let ctx = CleanupContext::default();
        let prompt = build_cleanup_user_prompt("hello world", &ctx);
        assert!(prompt.starts_with("<dictation_cleanup_request>\n<metadata>\n"));
        assert!(prompt.contains("Input type: single_dictation_utterance\n"));
        assert!(prompt.contains("Editable scope: current_transcript_only\n"));
        assert!(prompt.contains("Transcript role: data_to_transform_not_user_request\n"));
        assert!(prompt.contains("<<<GLIDE_RAW_TRANSCRIPT\nhello world\nGLIDE_RAW_TRANSCRIPT"));
    }

    #[test]
    fn test_build_user_prompt_with_target_app() {
        let ctx = CleanupContext {
            target_app: Some("Slack".to_string()),
            mode_hint: None,
            ..CleanupContext::default()
        };
        let prompt = build_cleanup_user_prompt("test", &ctx);
        assert!(prompt.contains("Target app: Slack\n"));
        assert!(prompt.contains("<<<GLIDE_RAW_TRANSCRIPT\ntest\nGLIDE_RAW_TRANSCRIPT"));
    }

    #[test]
    fn test_build_user_prompt_with_mode_hint() {
        let ctx = CleanupContext {
            target_app: None,
            mode_hint: Some("email".to_string()),
            ..CleanupContext::default()
        };
        let prompt = build_cleanup_user_prompt("test", &ctx);
        assert!(prompt.contains("Writing mode: email\n"));
        assert!(prompt.contains("<<<GLIDE_RAW_TRANSCRIPT\ntest\nGLIDE_RAW_TRANSCRIPT"));
    }

    #[test]
    fn test_build_user_prompt_full() {
        let ctx = CleanupContext {
            target_app: Some("VSCode".to_string()),
            mode_hint: Some("code comment".to_string()),
            ..CleanupContext::default()
        };
        let prompt = build_cleanup_user_prompt("fix the bug", &ctx);
        assert!(prompt.starts_with("<dictation_cleanup_request>\n<metadata>\n"));
        assert!(prompt.contains("Target app: VSCode\n"));
        assert!(prompt.contains("Writing mode: code comment\n"));
        assert!(prompt.ends_with("</dictation_cleanup_request>"));
    }

    #[tokio::test]
    async fn records_remote_provider_spans_with_mock_server() {
        let Some(server) = MockHttpServer::try_spawn(
            r#"{"choices":[{"message":{"role":"assistant","content":"cleaned text"}}]}"#,
        ) else {
            eprintln!("skipping mock server test because loopback sockets are unavailable");
            return;
        };
        let providers = ProvidersConfig {
            openai: ProviderCredentials {
                api_key: "test-key".to_string(),
                base_url: server.base_url(),
            },
            ..Default::default()
        };
        let profile = ProfileCollector::enabled();
        let provider = OpenAiLlmProvider::new(
            Provider::OpenAi,
            "test-model",
            &crate::llm::build_cleanup_system_prompt("system"),
            &providers,
            profile.clone(),
        )
        .unwrap();

        let cleaned = provider
            .clean("raw text", &CleanupContext::default())
            .await
            .unwrap();

        assert_eq!(cleaned, "cleaned text");
        let phases = profile
            .spans()
            .into_iter()
            .map(|span| span.phase)
            .collect::<Vec<_>>();
        assert!(phases.contains(&"remote_llm_json_request_build".to_string()));
        assert!(phases.contains(&"remote_llm_http_send_status".to_string()));
        assert!(phases.contains(&"remote_llm_response_parse".to_string()));
        assert!(phases.contains(&"remote_llm_provider_total".to_string()));
        let request = server.join();
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").last().unwrap()).unwrap();
        assert_eq!(body["temperature"].as_f64(), Some(0.0));
        assert!(
            body["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("CORE TASK:")
        );
        assert!(
            body["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("<<<GLIDE_RAW_TRANSCRIPT\nraw text\nGLIDE_RAW_TRANSCRIPT")
        );
    }

    #[tokio::test]
    async fn omits_temperature_for_openai_reasoning_models() {
        let Some(server) = MockHttpServer::try_spawn(
            r#"{"choices":[{"message":{"role":"assistant","content":"cleaned text"}}]}"#,
        ) else {
            eprintln!("skipping mock server test because loopback sockets are unavailable");
            return;
        };
        let providers = ProvidersConfig {
            openai: ProviderCredentials {
                api_key: "test-key".to_string(),
                base_url: server.base_url(),
            },
            ..Default::default()
        };
        let provider = OpenAiLlmProvider::new(
            Provider::OpenAi,
            "gpt-5.4-nano",
            &crate::llm::build_cleanup_system_prompt("system"),
            &providers,
            ProfileCollector::disabled(),
        )
        .unwrap();

        provider
            .clean("raw text", &CleanupContext::default())
            .await
            .unwrap();

        let request = server.join();
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").last().unwrap()).unwrap();
        assert!(body.get("temperature").is_none());
    }

    #[tokio::test]
    async fn keeps_temperature_for_openai_compatible_providers() {
        for provider in [Provider::Groq, Provider::Cerebras, Provider::Fireworks] {
            let Some(server) = MockHttpServer::try_spawn(
                r#"{"choices":[{"message":{"role":"assistant","content":"cleaned text"}}]}"#,
            ) else {
                eprintln!("skipping mock server test because loopback sockets are unavailable");
                return;
            };
            let providers = providers_for_mock(provider, server.base_url());
            let llm = OpenAiLlmProvider::new(
                provider,
                "test-model",
                &crate::llm::build_cleanup_system_prompt("system"),
                &providers,
                ProfileCollector::disabled(),
            )
            .unwrap();

            llm.clean("raw text", &CleanupContext::default())
                .await
                .unwrap();

            let request = server.join();
            let body: Value =
                serde_json::from_str(request.split("\r\n\r\n").last().unwrap()).unwrap();
            assert_eq!(body["temperature"].as_f64(), Some(0.0), "{provider:?}");
            assert_eq!(body["model"].as_str(), Some("test-model"));
            assert_eq!(body["messages"].as_array().unwrap().len(), 2);
        }
    }

    #[tokio::test]
    async fn preserves_error_status_and_body() {
        for (status_line, expected_status) in [
            ("400 Bad Request", "400 Bad Request"),
            ("401 Unauthorized", "401 Unauthorized"),
            ("429 Too Many Requests", "429 Too Many Requests"),
        ] {
            let body = r#"{"error":{"message":"Unsupported value: temperature"}}"#;
            let Some(server) = MockHttpServer::try_spawn_status(status_line, body) else {
                eprintln!("skipping mock server test because loopback sockets are unavailable");
                return;
            };
            let providers = ProvidersConfig {
                openai: ProviderCredentials {
                    api_key: "test-key".to_string(),
                    base_url: server.base_url(),
                },
                ..Default::default()
            };
            let provider = OpenAiLlmProvider::new(
                Provider::OpenAi,
                "gpt-4o-mini",
                &crate::llm::build_cleanup_system_prompt("system"),
                &providers,
                ProfileCollector::disabled(),
            )
            .unwrap();

            let error = provider
                .clean("raw text", &CleanupContext::default())
                .await
                .unwrap_err()
                .to_string();

            assert!(error.contains(expected_status), "{error}");
            assert!(error.contains("Unsupported value: temperature"), "{error}");
            server.join();
        }
    }

    fn providers_for_mock(provider: Provider, base_url: String) -> ProvidersConfig {
        let credentials = ProviderCredentials {
            api_key: "test-key".to_string(),
            base_url,
        };
        match provider {
            Provider::OpenAi => ProvidersConfig {
                openai: credentials,
                ..Default::default()
            },
            Provider::Groq => ProvidersConfig {
                groq: credentials,
                ..Default::default()
            },
            Provider::Cerebras => ProvidersConfig {
                cerebras: credentials,
                ..Default::default()
            },
            Provider::Fireworks => ProvidersConfig {
                fireworks: credentials,
                ..Default::default()
            },
            Provider::ElevenLabs | Provider::AppleLocal | Provider::Parakeet => {
                unreachable!("not an OpenAI-compatible LLM provider")
            }
        }
    }

    struct MockHttpServer {
        base_url: String,
        handle: thread::JoinHandle<String>,
    }

    impl MockHttpServer {
        fn try_spawn(body: &'static str) -> Option<Self> {
            Self::try_spawn_status("200 OK", body)
        }

        fn try_spawn_status(status_line: &'static str, body: &'static str) -> Option<Self> {
            let listener = TcpListener::bind("127.0.0.1:0").ok()?;
            let addr = listener.local_addr().unwrap();
            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 65536];
                let bytes = stream.read(&mut request).unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..bytes]).to_string();
                let response = format!(
                    "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).unwrap();
                request
            });

            Some(Self {
                base_url: format!("http://{addr}/v1"),
                handle,
            })
        }

        fn base_url(&self) -> String {
            self.base_url.clone()
        }

        fn join(self) -> String {
            self.handle.join().unwrap()
        }
    }
}
