use anyhow::{Context, Result};
use reqwest::{Client, multipart};
use serde::Deserialize;

use crate::{
    audio::AudioFormat,
    benchmark::ProfileCollector,
    config::{Provider, ProvidersConfig},
};

pub struct OpenAiSttProvider {
    provider: Provider,
    client: Client,
    endpoint: String,
    default_model: String,
    api_key: String,
    prompt: Option<String>,
    profile: ProfileCollector,
}

impl OpenAiSttProvider {
    pub fn new(
        provider: Provider,
        model: &str,
        providers: &ProvidersConfig,
        prompt: Option<String>,
        profile: ProfileCollector,
    ) -> Result<Self> {
        let creds = providers.credentials_for(provider);
        let api_key = creds.resolve_api_key("speech-to-text")?;
        let endpoint = provider.stt_endpoint_for_model(&creds.base_url, model);
        let model = if provider == Provider::Fireworks {
            model.rsplit('/').next().unwrap_or(model)
        } else {
            model
        };
        Ok(Self {
            provider,
            client: Client::new(),
            endpoint,
            default_model: model.to_string(),
            api_key,
            prompt,
            profile,
        })
    }
}

#[derive(Debug, Deserialize)]
struct OpenAiTranscriptionResponse {
    text: String,
}

#[async_trait::async_trait]
impl super::SttProvider for OpenAiSttProvider {
    async fn transcribe(&self, audio: &[u8], format: AudioFormat) -> Result<String> {
        let total_started = std::time::Instant::now();
        let mime = match format {
            AudioFormat::Wav => "audio/wav",
        };

        let request_started = std::time::Instant::now();
        let file_part = multipart::Part::bytes(audio.to_vec())
            .file_name("glide.wav")
            .mime_str(mime)
            .context("failed to create audio upload body")?;

        let mut form = multipart::Form::new()
            .text("model", self.default_model.clone())
            .part("file", file_part);

        if let Some(ref prompt) = self.prompt
            && !prompt.is_empty()
        {
            form = form.text("prompt", prompt.clone());
        }
        self.profile
            .record("remote_stt_request_body_build", request_started.elapsed());

        let send_started = std::time::Instant::now();
        self.profile
            .record_since_marker("stt_start", "stt_start_to_stt_http_send_start");
        self.profile
            .record_since_marker("flow_release", "flow_release_to_stt_http_send_start");
        let request = self.client.post(&self.endpoint).multipart(form);
        let request = if self.provider == Provider::Fireworks {
            request.header(reqwest::header::AUTHORIZATION, &self.api_key)
        } else {
            request.bearer_auth(&self.api_key)
        };
        let response = request
            .send()
            .await
            .context("failed to call transcription API")?
            .error_for_status()
            .context("transcription API returned an error status")?;
        self.profile
            .record("remote_stt_http_send_status", send_started.elapsed());

        let parse_started = std::time::Instant::now();
        let parsed: OpenAiTranscriptionResponse = response
            .json()
            .await
            .context("failed to parse transcription response")?;
        self.profile
            .record("remote_stt_response_parse", parse_started.elapsed());
        self.profile
            .record("remote_stt_provider_total", total_started.elapsed());

        Ok(parsed.text.trim().to_string())
    }

    fn name(&self) -> &'static str {
        "STT Provider"
    }
}
