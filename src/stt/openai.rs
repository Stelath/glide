use anyhow::{Context, Result};
use reqwest::{multipart, Client};
use serde::Deserialize;

use crate::{
    audio::AudioFormat,
    config::{GlideConfig, OpenAiSttConfig},
};

pub struct OpenAiSttProvider {
    client: Client,
    config: OpenAiSttConfig,
    api_key: String,
}

impl OpenAiSttProvider {
    pub fn new(config: GlideConfig) -> Result<Self> {
        let provider_config = config.stt.openai.clone();
        let api_key = provider_config.resolve_api_key()?;

        Ok(Self {
            client: Client::new(),
            config: provider_config,
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
            .text("model", self.config.model.clone())
            .part("file", file_part);

        let response = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("failed to call OpenAI transcription API")?
            .error_for_status()
            .context("OpenAI transcription API returned an error status")?;

        let parsed: OpenAiTranscriptionResponse = response
            .json()
            .await
            .context("failed to parse OpenAI transcription response")?;

        Ok(parsed.text.trim().to_string())
    }

    fn name(&self) -> &'static str {
        "OpenAI Whisper"
    }
}
