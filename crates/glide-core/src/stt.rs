use anyhow::{Context, Result};
use reqwest::{Client, multipart};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TranscribeConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    text: String,
}

pub async fn transcribe(audio: &[u8], config: &TranscribeConfig) -> Result<String> {
    let _ = &config.provider;
    let endpoint = format!(
        "{}/audio/transcriptions",
        config.base_url.trim_end_matches('/'),
    );

    let file_part = multipart::Part::bytes(audio.to_vec())
        .file_name("glide.wav")
        .mime_str("audio/wav")
        .context("failed to create audio upload body")?;

    let form = multipart::Form::new()
        .text("model", config.model.clone())
        .part("file", file_part);

    let response = Client::new()
        .post(&endpoint)
        .bearer_auth(&config.api_key)
        .multipart(form)
        .send()
        .await
        .context("failed to call transcription API")?
        .error_for_status()
        .context("transcription API returned an error status")?;

    let parsed: TranscriptionResponse = response
        .json()
        .await
        .context("failed to parse transcription response")?;

    Ok(parsed.text.trim().to_string())
}
