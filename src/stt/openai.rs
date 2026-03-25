use anyhow::{Context, Result};
use reqwest::{multipart, Client};
use serde::Deserialize;

use crate::{
    audio::AudioFormat,
    config::{Provider, ProvidersConfig},
};

pub struct OpenAiSttProvider {
    client: Client,
    endpoint: String,
    default_model: String,
    api_key: String,
}

impl OpenAiSttProvider {
    pub fn new(provider: Provider, model: &str, providers: &ProvidersConfig) -> Result<Self> {
        let creds = providers.credentials_for(provider);
        let api_key = creds.resolve_api_key("speech-to-text")?;
        let endpoint = provider.stt_endpoint(&creds.base_url);
        Ok(Self {
            client: Client::new(),
            endpoint,
            default_model: model.to_string(),
            api_key,
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
        let mime = match format {
            AudioFormat::Wav => "audio/wav",
        };

        let file_part = multipart::Part::bytes(audio.to_vec())
            .file_name("glide.wav")
            .mime_str(mime)
            .context("failed to create audio upload body")?;

        let form = multipart::Form::new()
            .text("model", self.default_model.clone())
            .part("file", file_part);

        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("failed to call transcription API")?
            .error_for_status()
            .context("transcription API returned an error status")?;

        let parsed: OpenAiTranscriptionResponse = response
            .json()
            .await
            .context("failed to parse transcription response")?;

        Ok(parsed.text.trim().to_string())
    }

    fn name(&self) -> &'static str {
        "STT Provider"
    }
}
