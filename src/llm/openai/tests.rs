use super::*;
use crate::{
    benchmark::ProfileCollector,
    config::{Provider, ProvidersConfig, providers::ProviderCredentials},
    llm::{CleanupContext, LlmProvider},
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

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
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").last().unwrap()).unwrap();
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
