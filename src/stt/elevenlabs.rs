use anyhow::{Context, Result};
use reqwest::{Client, multipart};
use serde::Deserialize;

use crate::{
    audio::AudioFormat,
    benchmark::ProfileCollector,
    config::{Provider, ProvidersConfig},
};

pub struct ElevenLabsSttProvider {
    client: Client,
    endpoint: String,
    model: String,
    api_key: String,
    profile: ProfileCollector,
}

impl ElevenLabsSttProvider {
    pub fn new(
        model: &str,
        providers: &ProvidersConfig,
        profile: ProfileCollector,
    ) -> Result<Self> {
        let creds = providers.credentials_for(Provider::ElevenLabs);
        let api_key = creds.resolve_api_key("ElevenLabs speech-to-text")?;
        let endpoint = Provider::ElevenLabs.stt_endpoint(&creds.base_url);
        Ok(Self {
            client: Client::new(),
            endpoint,
            model: model.to_string(),
            api_key,
            profile,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ElevenLabsTranscriptionResponse {
    text: String,
}

#[async_trait::async_trait]
impl super::SttProvider for ElevenLabsSttProvider {
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
        let form = multipart::Form::new()
            .text("model_id", self.model.clone())
            .text("file_format", "other")
            .part("file", file_part);
        self.profile
            .record("remote_stt_request_body_build", request_started.elapsed());

        let send_started = std::time::Instant::now();
        self.profile
            .record_since_marker("stt_start", "stt_start_to_stt_http_send_start");
        self.profile
            .record_since_marker("flow_release", "flow_release_to_stt_http_send_start");
        let response = self
            .client
            .post(&self.endpoint)
            .header("xi-api-key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .context("failed to call ElevenLabs speech-to-text API")?
            .error_for_status()
            .context("ElevenLabs speech-to-text API returned an error status")?;
        self.profile
            .record("remote_stt_http_send_status", send_started.elapsed());

        let parse_started = std::time::Instant::now();
        let parsed: ElevenLabsTranscriptionResponse = response
            .json()
            .await
            .context("failed to parse ElevenLabs transcription response")?;
        self.profile
            .record("remote_stt_response_parse", parse_started.elapsed());
        self.profile
            .record("remote_stt_provider_total", total_started.elapsed());

        Ok(parsed.text.trim().to_string())
    }

    fn name(&self) -> &'static str {
        "ElevenLabs STT Provider"
    }
}
